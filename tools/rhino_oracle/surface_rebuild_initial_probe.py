"""Closed, owned public Rebuild command recipes and complete surface data."""
import re


def surface(z,degree=(2,2),count=(3,3),rational=False,knots=None):
    def axis(p,n):return [0.]*(p+1)+[float(i)/(n-p)for i in range(1,n-p)]+[1.]*(p+1)
    ku,kv=(axis(degree[0],count[0]),axis(degree[1],count[1]))if knots is None else knots
    return dict(degree_u=degree[0],degree_v=degree[1],control_point_count_u=count[0],control_point_count_v=count[1],knots_u=ku,knots_v=kv,
        control_points=[dict(point=[4.*u/(count[0]-1),6.*v/(count[1]-1),z+u*v],weight=1.+.2*(u+v)if rational else 1.)for v in range(count[1])for u in range(count[0])])
SPECS={}
for name,definition,count,degree in [
 ('quadratic',surface(0.),(5,4),(3,2)),
 ('rational',surface(0.,rational=True),(5,4),(3,2)),
 ('curved',surface(0.,count=(4,4)),(4,4),(2,2)),
]:
    for delete,current in [(False,False),(True,True)]:
        SPECS[name+('_replace_current'if delete else '_copy_input')]=dict(
            sources=[definition],count=count,degree=degree,delete=delete,current=current)


def request():
    return dict(protocol_version=1, iterations=1, operations=[dict(op='surface_rebuild',id='tween_srf_'+case,case=case) for case in SPECS])


def validate_request(q):
    if (not isinstance(q,dict) or set(q)-{'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):
        raise ValueError('Rebuild requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='surface_rebuild'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or not isinstance(op['case'],str) or op['case'] not in SPECS):
            raise ValueError('invalid Rebuild recipe')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System']
    if Rhino.Commands.Command.InCommand():raise ValueError('Rebuild requires idle execution')
    doc,G=Rhino.RhinoDoc.ActiveDoc,Rhino.Geometry
    if list(doc.Objects):raise ValueError('Rebuild requires an empty owned document')
    from join_probe import observe_command
    saved_layer,saved_tolerance=doc.Layers.CurrentLayerIndex,doc.ModelAbsoluteTolerance
    doc.ModelAbsoluteTolerance=1e-6
    spec=SPECS[op['case']]
    ids,layers,groups=[],[],[]

    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda x:x.RuntimeSerialNumber):
            g,a=obj.Geometry,obj.Attributes
            s=g.Faces[0].UnderlyingSurface() if isinstance(g,G.Brep) else g
            rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                name=a.Name,layer=layers.index(a.LayerIndex) if a.LayerIndex in layers else None,
                selected=bool(obj.IsSelected(False)),groups=sorted(groups.index(i) for i in (a.GetGroupList() or []) if i in groups),
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                attribute_text=a.GetUserString('Code'),geometry_text=g.GetUserString('Code'),
                valid=bool(g.IsValid),faces=g.Faces.Count if isinstance(g,G.Brep) else 1,
                definition=host['_nurbs_surface_definition'](s),
                samples=[host['_xyz'](s.PointAt(s.Domain(0).ParameterAt(u/8.),s.Domain(1).ParameterAt(v/8.))) for v in range(9) for u in range(9)]))
        return rows

    def invoke(macro,command):
        marker='Viboceros Rebuild '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(Rhino.Commands.Command,command,lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        active=bool(Rhino.Commands.Command.InCommand())
        if active:Rhino.RhinoApp.RunScript('!',False)
        return dict(success=success,after=after,after_script=snapshot(),events=events,active=active,
                    history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1],macro=macro)

    try:
        for i in range(3):
            layer=Rhino.DocObjects.Layer();layer.Name='Viboceros TweenSurface '+str(System.Guid.NewGuid())
            try:layers.append(doc.Layers.Add(layer))
            finally:layer.Dispose()
        for i,definition in enumerate(spec['sources']):
            g=host['_nurbs_surface_from_definition'](definition);a=Rhino.DocObjects.ObjectAttributes()
            try:
                a.Name,a.LayerIndex='source-'+str(i),layers[i]
                a.ObjectColor=System.Drawing.Color.FromArgb(20+i,40,60);a.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
                a.SetUserString('Code','attribute-'+str(i));g.SetUserString('Code','geometry-'+str(i))
                ids.append(doc.Objects.AddSurface(g,a))
                if ids[-1]==System.Guid.Empty:raise ValueError('Rebuild insertion failed')
            finally:g.Dispose();a.Dispose()
        for members in [[key] for key in ids]+[ids]:groups.append(doc.Groups.Add('Viboceros TweenSurface '+str(System.Guid.NewGuid()),members))
        doc.Layers.SetCurrentLayerIndex(layers[2],True);doc.ClearUndoRecords(True)
        before=snapshot()
        macro='_-Rebuild '+' '.join('_SelID '+str(key)for key in ids)+' _Enter'
        macro+=' _PointCount='+str(spec['count'][0])+','+str(spec['count'][1])
        macro+=' _Degree='+str(spec['degree'][0])+','+str(spec['degree'][1])
        macro+=' _DeleteInput='+('Yes'if spec['delete']else 'No')
        macro+=' _CurrentLayer='+('Yes'if spec['current']else 'No')+' _ReTrim=No _Enter'
        source=host['_nurbs_surface_from_definition'](spec['sources'][0])
        try:
            rebuilt=source.Rebuild(spec['degree'][0],spec['degree'][1],spec['count'][0],spec['count'][1])
            if rebuilt is None:raise ValueError('surface rebuild SDK failed')
            try:prepared=host['_nurbs_surface_definition'](rebuilt)
            finally:rebuilt.Dispose()
        finally:source.Dispose()
        result=dict(case=op['case'],spec=spec,before=before,rebuild_sdk=prepared,command=invoke(macro,'Rebuild'))
        result['undo']=invoke('_Undo','Undo');result['redo']=invoke('_Redo','Redo')
        return result,0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for index in groups:doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        doc.ModelAbsoluteTolerance=saved_tolerance
