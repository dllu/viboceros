"""Independent public SDK length and projection diagnostics on saved sources."""
import math
import re


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','artifact_path','station'}
            or op.get('op')!='soft_edit_curve_lengths' or not isinstance(op.get('id'),str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id'])is None
            or not isinstance(op['artifact_path'],str) or not op['artifact_path'].endswith('.3dm')
            or type(op['station'])not in(int,float) or not 0<=op['station']<=1):
        raise ValueError('invalid soft edit length recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one SDK observation per source')
    R=host['Rhino'];G=R.Geometry
    path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
    file=R.FileIO.File3dm.Read(path)
    if file is None:raise ValueError('source read failed')
    curve=None
    try:
        objects=list(file.Objects)
        if len(objects)!=1:raise ValueError('expected one source curve')
        source=objects[0].Geometry
        reference=source.PointAt(source.Domain.ParameterAt(op['station']))
        curve=source.ToNurbsCurve()
        if curve is None:raise ValueError('source NURBS conversion failed')
        ok,projected=curve.ClosestPoint(reference)
        if not ok:raise ValueError('source projection failed')
        sites=[float(curve.GrevilleParameter(i))for i in range(curve.Points.Count)]
        domain=curve.Domain
        valid=[t for t in sites if domain.T0<=t<=domain.T1]
        # The repeated seam endpoint is represented by the first marker.
        markers=list(valid)
        if curve.IsClosed and markers and curve.PointAt(markers[0]).DistanceTo(curve.PointAt(markers[-1]))<=2.3283064365386963e-10:markers.pop()
        anchor=min(markers,key=lambda t:(abs(t-projected),-t))
        snapped=curve.PointAt(anchor);success,back=curve.ClosestPoint(snapped)
        rows=[]
        for i,t in enumerate(sites):
            if not domain.T0<=t<=domain.T1:continue
            if t==anchor:default=accurate=explicit=0.
            else:
                interval=G.Interval(min(t,anchor),max(t,anchor))
                default=float(curve.GetLength(interval))
                explicit=float(curve.GetLength(1e-8,interval))
                accurate=float(curve.GetLength(1e-12,interval))
            if any(math.isnan(x)or math.isinf(x)or x<0 for x in(default,explicit,accurate)):
                raise ValueError('invalid SDK length')
            rows.append(dict(index=i,parameter=t,default=default,explicit_1e8=explicit,accurate=accurate))
        return dict(definition=host['_nurbs_curve_definition'](curve),reference=host['_xyz'](reference),
            projection=float(projected),anchor_parameter=anchor,anchor=host['_xyz'](snapped),
            snapped_projection=float(back),snapped_projection_success=bool(success),periodic=bool(curve.IsPeriodic),
            closed=bool(curve.IsClosed),lengths=rows),0
    finally:
        if curve is not None:curve.Dispose()
        file.Dispose()
