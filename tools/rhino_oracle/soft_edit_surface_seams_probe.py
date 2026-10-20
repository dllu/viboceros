"""Command validity, seam gaps and UV/spatial boundaries through the public SDK."""
try:
    from . import soft_edit_surface_shapes_probe as base
except (ImportError,ValueError):
    import soft_edit_surface_shapes_probe as base


def mapped(op):return dict((k,v)for k,v in dict(op,op='soft_edit_surface_shapes').items()if k!='skew_v')
def validate(op):
    if not isinstance(op,dict)or op.get('op')!='soft_edit_surface_seams':raise ValueError('invalid seam recipe')
    base.validate(mapped(op))
    if type(op.get('skew_v',False))is not bool:raise ValueError('invalid chart flag')


def run(op,host,iterations,tolerance):
    validate(op)
    import join_probe
    R=host['Rhino'];G=R.Geometry;original=join_probe.observe_command;state={}
    def curve_record(curve):
        n=curve.ToNurbsCurve()
        try:return host['_interchange_curve_record'](n)
        finally:n.Dispose()
    def record():
        rows=[]
        for o in R.RhinoDoc.ActiveDoc.Objects:
            geometry=o.Geometry
            if not isinstance(geometry,G.Brep):raise ValueError('expected surface B-rep')
            valid,log=geometry.IsValidWithLog()
            surface=geometry.Faces[0].UnderlyingSurface();du,dv=surface.Domain(0),surface.Domain(1)
            gaps=[]
            for axis in(0,1):
                pairs=[(surface.PointAt(du.T0,dv.ParameterAt(t/64.)),surface.PointAt(du.T1,dv.ParameterAt(t/64.)))if axis==0 else(surface.PointAt(du.ParameterAt(t/64.),dv.T0),surface.PointAt(du.ParameterAt(t/64.),dv.T1))for t in range(65)]
                gaps.append(max(a.DistanceTo(b)for a,b in pairs))
            trims=[]
            for face in geometry.Faces:
                for loop in face.Loops:
                    for trim in loop.Trims:
                        domain=trim.Domain;uv=[trim.PointAt(domain.ParameterAt(i/16.))for i in range(17)]
                        edge=trim.Edge
                        trims.append(dict(type=str(trim.TrimType),uv_curve=curve_record(trim),uv=[host['_xyz'](p)for p in uv],surface_points=[host['_xyz'](face.PointAt(p.X,p.Y))for p in uv],edge_points=[host['_xyz'](edge.PointAt(edge.Domain.ParameterAt(i/16.)))for i in range(17)]if edge is not None else None))
            rebuilt=surface.ToBrep()
            try:rebuilt_valid,rebuilt_log=rebuilt.IsValidWithLog()
            finally:rebuilt.Dispose()
            rows.append(dict(id=str(o.Id),valid=bool(valid),validity_log=str(log),closed=[bool(surface.IsClosed(i))for i in(0,1)],periodic=[bool(surface.IsPeriodic(i))for i in(0,1)],seam_gaps=gaps,trims=trims,rebuilt_valid=bool(rebuilt_valid),rebuilt_log=str(rebuilt_log),edge_tolerances=[float(e.Tolerance)for e in geometry.Edges],vertex_tolerances=[float(v.Tolerance)for v in geometry.Vertices]))
        return rows
    def observe(event_source,command,execute,*args,**kwargs):
        result=original(event_source,command,execute,*args,**kwargs)
        if command=='SoftEditSrf':state['accepted_validity']=record()
        return result
    join_probe.observe_command=observe
    try:
        local=dict(host)
        if op.get('skew_v'):
            factory=host['_nurbs_surface_from_definition']
            def source(definition):
                d=dict(definition);knots=d['knots_v'];unique=sorted(set(knots))
                if len(unique)!=5:raise ValueError('skew chart requires four Bezier spans')
                targets=[unique[0]+(unique[-1]-unique[0])*f for f in(0.,.6,.8,.9,1.)];mapping=dict(zip(unique,targets));d['knots_v']=[mapping[t]for t in knots];return factory(d)
            local['_nurbs_surface_from_definition']=source
        value,elapsed=base.run(mapped(op),local,iterations,tolerance);value['recipe']=op;value.update(state);value['model_tolerance']=float(R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance);return value,elapsed
    finally:join_probe.observe_command=original
