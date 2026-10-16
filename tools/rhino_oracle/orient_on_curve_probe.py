"""Owned OrientOnCrv placements with a four-point affine witness cloud."""
import math
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source

def validate(op):
    if not isinstance(op,dict)or not {'op','id','source','base','target','options'}<=set(op)or set(op)-{'op','id','source','base','target','options','plane'}or op['op']!='orient_on_curve':raise ValueError('invalid OrientOnCrv recipe')
    validate_source(dict(op='geometry_validity',id=op['id'],source=op['source']))
    if op['source']['type']not in ('line','arc','circle','ellipse','nurbs','polyline','polycurve'):raise ValueError('orientation curve required')
    for k in ('base','target'):
        if not isinstance(op[k],list)or len(op[k])!=3 or any(type(v)not in (int,float)or math.isnan(v)or math.isinf(v)for v in op[k]):raise ValueError('invalid orientation point')
    if not isinstance(op['options'],dict)or set(op['options'])-{'copy','perpendicular','flip','xflip','yflip','rotate','on_curve'}:raise ValueError('invalid orientation options')
    if op.get('plane','top')not in ('top','front','right'):raise ValueError('invalid placement construction plane')
    for key,value in op['options'].items():
        if key=='rotate':
            if type(value)not in (int,float)or not -3600<=value<=3600:raise ValueError('invalid orientation rotation')
        elif type(value)is not bool:raise ValueError('invalid orientation toggle')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('owned placement requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    old_plane=vp.GetConstructionPlane().Plane;before_doc=set(o.Id for o in doc.Objects)
    curve=host['_object_source'](op['source'],tolerance);base=op['base'];cloud=G.PointCloud([G.Point3d(*(base[j]+(1 if i==j+1 else 0)for j in range(3)))for i in range(4)])
    def point(p):return 'w'+','.join(repr(float(x))for x in p)
    try:
        vp.SetConstructionPlane({'top':G.Plane.WorldXY,'front':G.Plane.WorldZX,'right':G.Plane.WorldYZ}[op.get('plane','top')])
        key=doc.Objects.AddCurve(curve);source=doc.Objects.AddPointCloud(cloud)
        before=host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry)
        doc.Objects.UnselectAll();doc.Objects.Select(source,True)
        options=[]
        for name,value in sorted(op['options'].items()):
            if name=='copy':options.append('_Copy=_'+('Yes'if value else'No'))
            elif name=='rotate':options.append('_Rotate=_Yes')
            elif name=='on_curve':continue
            elif value:options.append('_'+name)
        marker='Viboceros OrientOnCrv '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        start=('_OnCurve _SelID '+str(key)+' '+point(base))if op['options'].get('on_curve')else(point(base)+' _SelID '+str(key))
        macro='_OrientOnCrv '+start+' '+' '.join(options)+' '+point(op['target'])+(' '+repr(float(op['options']['rotate']))if'rotate'in op['options']else'')+(' _Enter'if op['options'].get('copy')else'')+' _Cancel'
        ran=R.RhinoApp.RunScript(macro,True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
        outputs=[]
        for obj in doc.Objects:
            if obj.Id in before_doc or obj.Id==key:continue
            if isinstance(obj.Geometry,G.PointCloud):outputs.append(dict(original=obj.Id==source,selected=bool(obj.IsSelected(False)),points=[host['_xyz'](p.Location)for p in obj.Geometry]))
        after=host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry)
        def plane_value(p):return dict(origin=host['_xyz'](p.Origin),x=host['_xyz'](p.XAxis),y=host['_xyz'](p.YAxis),z=host['_xyz'](p.ZAxis))
        ok,t=curve.ClosestPoint(G.Point3d(*op['target']));frames={}
        if ok:
            for name,method in [('frame',curve.FrameAt),('perp',curve.PerpendicularFrameAt)]:
                valid,p=method(t)
                if valid:frames[name]=plane_value(p)
            valid,p=curve.TryGetPlane()
            if valid:frames['curve_plane']=plane_value(p)
        return dict(frames=frames,command_returned=bool(ran),history=history,macro=macro,outputs=outputs,curve_unchanged=before==after,curve_selected=bool(doc.Objects.FindId(key).IsSelected(False))),0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before_doc:doc.Objects.Delete(obj.Id,True)
        vp.SetConstructionPlane(old_plane);cloud.Dispose();curve.Dispose()
