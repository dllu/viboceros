"""Actual point-based Orient commands with an affine witness cloud."""
import math
import re


def validate(op):
    required={'op','id','command','references','targets','scale','copy'}
    if not isinstance(op,dict)or not required<=set(op)or set(op)-required-{'postselected','group','plane','batch_targets','default_target','edit_scale','setup_scale','omit_scale','cancel_after'}or op['op']!='orient_points':raise ValueError('invalid point orientation recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid point orientation id')
    if op['command']not in('Orient','Orient3Pt'):raise ValueError('invalid point orientation command')
    n=2 if op['command']=='Orient'else 3
    for key in('references','targets'):
        if not isinstance(op[key],list)or len(op[key])!=n:raise ValueError('wrong orientation point count')
        for p in op[key]:
            if not isinstance(p,list)or len(p)!=3 or any(type(x)not in(int,float)or (math.isnan(x)or math.isinf(x))for x in p):raise ValueError('invalid orientation point')
    if op['scale']not in(('No','1D','3D')if n==2 else('No','Yes')):raise ValueError('invalid orientation scale')
    for key in('copy','postselected','group','default_target','omit_scale'):
        if type(op.get(key,False))is not bool:raise ValueError('invalid orientation flag')
    for key in('edit_scale','setup_scale'):
        if key in op and op[key]not in(('No','1D','3D')if n==2 else('No','Yes')):raise ValueError('invalid staged scale')
    if op.get('omit_scale')and'setup_scale'not in op:raise ValueError('omitted scale needs an explicit setup')
    if op.get('default_target')and(n!=2 or op['targets'][0]!=op['references'][0]):raise ValueError('invalid target default')
    if 'cancel_after'in op and(type(op['cancel_after'])is not int or not 0<=op['cancel_after']<2*n):raise ValueError('invalid cancellation phase')
    batches=op.get('batch_targets',[])
    if not isinstance(batches,list)or len(batches)>3 or batches and(not op['copy']or'cancel_after'in op):raise ValueError('invalid orientation batch')
    for targets in batches:
        changed=dict(op);changed.pop('batch_targets',None);changed['targets']=targets;changed.pop('default_target',None);validate(changed)
    if op.get('plane','top')not in('top','front','right'):raise ValueError('invalid orientation plane')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('point orientation requires one iteration')
    n=2 if op['command']=='Orient'else 3
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    before=set(o.Id for o in doc.Objects);before_groups=set(g.Index for g in doc.Groups if not g.IsDeleted);old_plane=vp.GetConstructionPlane().Plane
    base=op['references'][0]
    cloud=G.PointCloud([G.Point3d(*(base[j]+(1. if i==j+1 else 0.)for j in range(3)))for i in range(4)])
    group=-1
    try:
        vp.SetConstructionPlane({'top':G.Plane.WorldXY,'front':G.Plane.WorldZX,'right':G.Plane.WorldYZ}[op.get('plane','top')])
        source=doc.Objects.AddPointCloud(cloud)
        if op.get('group'):group=doc.Groups.Add('Owned orient source', [source])
        doc.Objects.UnselectAll()
        if not op.get('postselected'):doc.Objects.Select(source,True)
        marker='Viboceros point orient '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        pick=('_SelID '+str(source)+' _Enter ')if op.get('postselected')else''
        if 'setup_scale'in op:R.RhinoApp.RunScript('_'+op['command']+' _Scale=_'+op['setup_scale']+' _Cancel',True)
        options='_Copy=_'+('Yes'if op['copy']else'No')+' '+(''if op.get('omit_scale')else'_Scale=_'+op['scale']+' ')
        point=lambda p:'w'+','.join(repr(float(x))for x in p)
        points=[point(p)for p in op['references']+op['targets']]
        if op.get('default_target'):points[n]='_Enter'
        if 'edit_scale'in op:points.insert(n,'_Scale=_'+op['edit_scale'])
        if 'cancel_after'in op:points=points[:op['cancel_after']]
        for targets in op.get('batch_targets',[]):points.extend(point(p)for p in targets)
        macro='_'+op['command']+' '+pick+options+' '.join(points)+' _Cancel'
        returned=R.RhinoApp.RunScript(macro,True)
        outputs=[]
        for obj in doc.Objects:
            if obj.Id in before or not isinstance(obj.Geometry,G.PointCloud):continue
            outputs.append(dict(original=obj.Id==source,selected=bool(obj.IsSelected(False)),points=[host['_xyz'](p.Location)for p in obj.Geometry],group_count=obj.GroupCount))
        return dict(command_returned=bool(returned),macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip(),outputs=outputs,group_count=sum(not g.IsDeleted and g.Index not in before_groups for g in doc.Groups)),0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:doc.Objects.Delete(obj.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted and g.Index not in before_groups:doc.Groups.Delete(g.Index)
        cloud.Dispose();vp.SetConstructionPlane(old_plane)
