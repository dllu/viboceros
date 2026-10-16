"""Owned OrientOnCrv placements with a four-point affine witness cloud."""
import math
import re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source

def validate(op):
    if not isinstance(op,dict)or not {'op','id','source','base','target','options'}<=set(op)or set(op)-{'op','id','source','base','target','options','plane','subcurve','relative_target','edge','edge_anchor'}or op['op']!='orient_on_curve':raise ValueError('invalid OrientOnCrv recipe')
    validate_source(dict(op='geometry_validity',id=op['id'],source=op['source']))
    if 'edge'in op and(type(op['edge'])is not int or not 0<=op['edge']<64):raise ValueError('invalid guide edge index')
    if 'edge'in op and re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid edge case id')
    if 'edge'in op and op['source']['type']not in ('box_brep','surface','cylinder_brep'):raise ValueError('surface guide parent required')
    if 'edge'not in op and op['source']['type']not in ('line','arc','circle','ellipse','nurbs','polyline','polycurve'):raise ValueError('orientation curve required')
    if 'edge_anchor'in op and('edge'not in op or op['source']['type']not in('box_brep','cylinder_brep')):raise ValueError('edge anchor requires a B-rep guide')
    for k in ('base','target')+(('relative_target',)if'relative_target'in op else())+(('edge_anchor',)if'edge_anchor'in op else()):
        if not isinstance(op[k],list)or len(op[k])!=3 or any(type(v)not in (int,float)or math.isnan(v)or math.isinf(v)for v in op[k]):raise ValueError('invalid orientation point')
    if not isinstance(op['options'],dict)or set(op['options'])-{'copy','perpendicular','flip','xflip','yflip','rotate','on_curve'}:raise ValueError('invalid orientation options')
    if op.get('plane','top')not in ('top','front','right'):raise ValueError('invalid placement construction plane')
    for key,value in op['options'].items():
        if key=='rotate':
            if type(value)not in (int,float)or not -3600<=value<=3600:raise ValueError('invalid orientation rotation')
        elif type(value)is not bool:raise ValueError('invalid orientation toggle')
    if 'relative_target'in op and(not op['options'].get('copy')or'rotate'in op['options']):raise ValueError('relative followup requires Copy without rotation')
    if 'subcurve' in op:
        sub=op['subcurve']
        if not isinstance(sub,dict) or not {'start','end'}<=set(sub) or set(sub)-{'start','end','length'}:raise ValueError('invalid temporary orientation curve')
        for key in ('start','end'):
            if not isinstance(sub[key],list) or len(sub[key])!=3 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in sub[key]):raise ValueError('invalid temporary curve point')
        if 'length'in sub and(type(sub['length'])not in(int,float)or math.isnan(sub['length'])or math.isinf(sub['length'])or sub['length']==0):raise ValueError('invalid temporary curve length')
    if 'edge'in op and('relative_target'in op or 'length'in op.get('subcurve',{})):raise ValueError('edge driver supports point endpoints and one placement')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('owned placement requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    old_view=R.DocObjects.ViewportInfo(vp)if'edge'in op else None
    old_plane=vp.GetConstructionPlane().Plane;before_doc=set(o.Id for o in doc.Objects)
    curve=host['_object_source'](op['source'],tolerance);base=op['base'];cloud=G.PointCloud([G.Point3d(*(base[j]+(1 if i==j+1 else 0)for j in range(3)))for i in range(4)])
    def point(p):return 'w'+','.join(repr(float(x))for x in p)
    try:
        vp.SetConstructionPlane({'top':G.Plane.WorldXY,'front':G.Plane.WorldZX,'right':G.Plane.WorldYZ}[op.get('plane','top')])
        parent=curve
        if 'edge'in op:
            if isinstance(parent,G.Surface):
                key=doc.Objects.AddSurface(parent);brep=parent.ToBrep()
            elif isinstance(parent,G.Brep):key=doc.Objects.AddBrep(parent);brep=parent
            else:raise ValueError('surface guide parent required')
            edge_index=op['edge']
            if 'edge_anchor'in op:
                anchor=G.Point3d(*op['edge_anchor']);candidates=[]
                for i,e in enumerate(brep.Edges):
                    ok,t=e.ClosestPoint(anchor)
                    if ok:candidates.append((e.PointAt(t).DistanceTo(anchor),i))
                if not candidates:raise ValueError('no available edge at anchor')
                edge_index=min(candidates)[1]
            curve=brep.Edges[edge_index].DuplicateCurve()
            if brep is not parent:brep.Dispose()
        else:key=doc.Objects.AddCurve(curve)
        source=doc.Objects.AddPointCloud(cloud)
        def definition(g):
            if isinstance(g,G.Brep):return host['_interchange_brep_record'](g,include_samples=False)
            if isinstance(g,G.Surface):
                surface=g.ToNurbsSurface()
                try:return host['_nurbs_surface_definition'](surface)
                finally:surface.Dispose()
            return host['_nurbs_curve_definition'](g)
        before=definition(doc.Objects.FindId(key).Geometry)
        doc.Objects.UnselectAll();doc.Objects.Select(source,True)
        options=[]
        for name,value in sorted(op['options'].items()):
            if name=='copy':options.append('_Copy=_'+('Yes'if value else'No'))
            elif name=='rotate':options.append('_Rotate=_Yes')
            elif name=='on_curve':continue
            elif value:options.append('_'+name)
        marker='Viboceros OrientOnCrv '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        pick='_SelID '+str(key)
        if 'subcurve'in op:
            sub=op['subcurve'];pick='_SubCrv '+pick+' '+point(sub['start'])+' '+(repr(float(sub['length']))+' 'if'length'in sub else'')+point(sub['end'])
        start=('_OnCurve '+pick+' '+point(base))if op['options'].get('on_curve')else(point(base)+' '+pick)
        macro='_OrientOnCrv '+start+' '+' '.join(options)+' '+point(op['target'])+(' '+repr(float(op['options']['rotate']))if'rotate'in op['options']else'')+(' rw'+','.join(repr(float(v))for v in op['relative_target'])if'relative_target'in op else'')+(' _Enter'if op['options'].get('copy')else'')+' _Cancel'
        if 'edge'in op:host['_record_progress']('edge-guide: parent snapshot complete')
        edge_input=None
        if 'edge'in op:
            from orient_on_curve_edge_input import drive
            macro='_OrientOnCrv _Pause'
            ran,edge_input=drive(op,host,key,edge_index,point)
        else:ran=R.RhinoApp.RunScript(macro,True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip()
        outputs=[]
        for obj in doc.Objects:
            if obj.Id in before_doc or obj.Id==key:continue
            if isinstance(obj.Geometry,G.PointCloud):outputs.append(dict(original=obj.Id==source,selected=bool(obj.IsSelected(False)),points=[host['_xyz'](p.Location)for p in obj.Geometry]))
        after=definition(doc.Objects.FindId(key).Geometry)
        def plane_value(p):return dict(origin=host['_xyz'](p.Origin),x=host['_xyz'](p.XAxis),y=host['_xyz'](p.YAxis),z=host['_xyz'](p.ZAxis))
        ok,t=curve.ClosestPoint(G.Point3d(*op['target']));frames={}
        if ok:
            for name,method in [('frame',curve.FrameAt),('perp',curve.PerpendicularFrameAt)]:
                valid,p=method(t)
                if valid:frames[name]=plane_value(p)
            valid,p=curve.TryGetPlane()
            if valid:frames['curve_plane']=plane_value(p)
        return dict(guide_edge_index=edge_index if'edge'in op else None,edge_input=edge_input,guide_definition=host['_nurbs_curve_definition'](curve)if'edge'in op else None,frames=frames,command_returned=bool(ran),history=history,macro=macro,outputs=outputs,curve_unchanged=before==after,curve_selected=bool(doc.Objects.FindId(key).IsSelected(False))),0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before_doc:doc.Objects.Delete(obj.Id,True)
        cloud.Dispose();curve.Dispose()
        if 'edge'in op:
            if parent is not curve:parent.Dispose()
            vp.SetViewProjection(old_view,False);old_view.Dispose()
        vp.SetConstructionPlane(old_plane)
