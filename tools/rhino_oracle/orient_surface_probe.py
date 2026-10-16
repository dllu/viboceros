"""Actual OrientOnSrf command input with authored point-cloud witnesses."""
import math
import re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_geometry
else:
    from geometry_validity_probe import validate as validate_geometry

def validate(op):
    if not isinstance(op,dict)or set(op)-{'op','id','surface','base','reference','uv','options','plane','postselected','group','reverse_face','as_brep','batch','offsets'}or not {'op','id','surface','base','reference','uv','options'}<=set(op)or op['op']!='orient_surface':raise ValueError('invalid surface orientation recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid surface orientation id')
    if not isinstance(op['surface'],dict)or op['surface'].get('type')!='surface':raise ValueError('NURBS target recipe required')
    validate_geometry(dict(op='geometry_validity',id=op['id'],source=op['surface']))
    for key,n in [('base',3),('reference',3),('uv',2)]:
        if not isinstance(op[key],list)or len(op[key])!=n or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in op[key]):raise ValueError('invalid orientation coordinates')
    if not isinstance(op['options'],dict)or set(op['options'])-{'copy','rigid','flip','scale','rotation','constrain_normal','on_surface'}:raise ValueError('invalid surface orientation options')
    for key,value in op['options'].items():
        if key in('scale','rotation'):
            if type(value)not in(int,float)or math.isnan(value)or math.isinf(value)or key=='scale'and value<=0:raise ValueError('invalid orientation number')
        elif type(value)is not bool:raise ValueError('invalid orientation toggle')
    for entries,name,size,minimum,maximum in [(op.get('offsets',[[0,0,0],[1,0,0],[0,1,0],[0,0,1]]),'offsets',3,4,32)]:
        if not isinstance(entries,list)or not minimum<=len(entries)<=maximum:raise ValueError('invalid '+name+' count')
        for point in entries:
            if not isinstance(point,list)or len(point)!=size or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in point):raise ValueError('invalid '+name+' coordinates')
    batch=op.get('batch',[])
    if not isinstance(batch,list)or len(batch)>3 or batch and not op['options'].get('copy',True):raise ValueError('invalid placement batch')
    for b in batch:
        if not isinstance(b,dict)or not {'uv'}<=set(b)or set(b)-{'uv','scale','rotation'}:raise ValueError('invalid batch placement')
        if not isinstance(b['uv'],list)or len(b['uv'])!=2 or any(type(x)not in(int,float)or not 0<=x<=1 for x in b['uv']):raise ValueError('invalid batch parameters')
        for k in('scale','rotation'):
            x=b.get(k,op['options'].get(k,1 if k=='scale'else 0))
            if type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or k=='scale'and x<=0:raise ValueError('invalid batch scalar')
    if op['options'].get('rigid',True)and op['options'].get('constrain_normal'):raise ValueError('constrained normal requires deformation')
    if op.get('plane','top')not in('top','front','right'):raise ValueError('invalid source plane')
    for key in('postselected','group','reverse_face','as_brep'):
        if type(op.get(key,False))is not bool:raise ValueError('invalid selection flag')

def run(op,host,iterations,tolerance,macro_builder=None):
    validate(op)
    if iterations!=1:raise ValueError('surface orientation requires one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    before=set(o.Id for o in doc.Objects);before_groups=set(g.Index for g in doc.Groups if not g.IsDeleted);old_plane=vp.GetConstructionPlane().Plane
    surface=host['_object_source'](op['surface'],tolerance);base=op['base'];cloud=G.PointCloud([G.Point3d(*(base[j]+offset[j]for j in range(3)))for offset in op.get('offsets',[[0,0,0],[1,0,0],[0,1,0],[0,0,1]])])
    try:
        vp.SetConstructionPlane({'top':G.Plane.WorldXY,'front':G.Plane.WorldZX,'right':G.Plane.WorldYZ}[op.get('plane','top')])
        if op.get('reverse_face')or op.get('as_brep'):
            parent=surface.ToBrep()
            if op.get('reverse_face'):parent.Flip()
            try:guide=doc.Objects.AddBrep(parent)
            finally:parent.Dispose()
        else:guide=doc.Objects.AddSurface(surface)
        source=doc.Objects.AddPointCloud(cloud)
        definition=lambda:host['_interchange_brep_record'](doc.Objects.FindId(guide).Geometry,include_samples=False)
        before_guide=definition()
        if op.get('group'):doc.Groups.Add('Owned surface orientation source',[source])
        doc.Objects.UnselectAll()
        if not op.get('postselected'):doc.Objects.Select(source,True)
        target=surface.PointAt(*op['uv']);options=op['options'];point=lambda p:'w'+','.join(repr(float(x))for x in p)
        pick="'_-SelID "+str(guide)+' '
        prefix=('_SelID '+str(source)+' _Enter ')if op.get('postselected')else''
        refs=point(base)+' '+point(op['reference'])+' '+pick
        if options.get('on_surface'):refs='_OnSurface '+pick+point(base)+' '+point(op['reference'])+' '
        flags='_Copy=_'+('Yes'if options.get('copy',True)else'No')+' _Rigid=_'+('Yes'if options.get('rigid',True)else'No')+' _IgnoreTrims=_No _Flip=_'+('Yes'if options.get('flip')else'No')+' '
        if not options.get('rigid',True):flags+='_ConstrainNormal=_'+('Yes'if options.get('constrain_normal')else'No')+' '
        marker='Owned OrientOnSrf '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-OrientOnSrf '+prefix+refs+flags+point(host['_xyz'](target))+' '+repr(float(options.get('scale',1)))+' '+repr(float(options.get('rotation',0)))+(' _Enter'if options.get('copy',True)else'')+' _Cancel'
        if op.get('batch'):
            macro=macro.rsplit(' _Enter',1)[0]
            for entry in op['batch']:
                q=surface.PointAt(*entry['uv']);macro+=' '+point(host['_xyz'](q))+' '+repr(float(entry.get('scale',options.get('scale',1))))+' '+repr(float(entry.get('rotation',options.get('rotation',0))))
            macro+=' _Enter _Cancel'
        if macro_builder is not None:macro=macro_builder(op,macro)
        returned=R.RhinoApp.RunScript(macro,True)
        outputs=[]
        for obj in doc.Objects:
            if obj.Id in before or obj.Id==guide or not isinstance(obj.Geometry,G.PointCloud):continue
            outputs.append(dict(original=obj.Id==source,selected=bool(obj.IsSelected(False)),points=[host['_xyz'](p.Location)for p in obj.Geometry],group_count=obj.GroupCount))
        return dict(guide_unchanged=before_guide==definition(),command_returned=bool(returned),history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip(),macro=macro,outputs=outputs,group_count=sum(not g.IsDeleted and g.Index not in before_groups for g in doc.Groups),guide_selected=bool(doc.Objects.FindId(guide).IsSelected(False))),0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:doc.Objects.Delete(obj.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted and g.Index not in before_groups:doc.Groups.Delete(g.Index)
        surface.Dispose();cloud.Dispose();vp.SetConstructionPlane(old_plane)
