"""Closed, owned public TweenSurfaces command recipes and complete surface data."""
import re


def surface(z,degree=(2,2),count=(3,3),rational=False,knots=None):
    def axis(p,n):return [0.]*(p+1)+[float(i)/(n-p)for i in range(1,n-p)]+[1.]*(p+1)
    ku,kv=(axis(degree[0],count[0]),axis(degree[1],count[1]))if knots is None else knots
    return dict(degree_u=degree[0],degree_v=degree[1],control_point_count_u=count[0],control_point_count_v=count[1],knots_u=ku,knots_v=kv,
        control_points=[dict(point=[4.*u/(count[0]-1),6.*v/(count[1]-1),z+u*v],weight=1.+.2*(u+v)if rational else 1.)for v in range(count[1])for u in range(count[0])])
SPECS={}
for family,sources in {
 'counts':[surface(0.),surface(4.,count=(4,4))],
 'counts_swapped':[surface(4.,count=(4,4)),surface(0.)],
 'u_only':[surface(0.),surface(4.,count=(4,3))],
 'v_only':[surface(0.),surface(4.,count=(3,4))],
 'degrees':[surface(0.,degree=(1,1),count=(2,2)),surface(4.)],
 'one_degree':[surface(0.,degree=(1,2),count=(2,3)),surface(4.)],
 'rational_counts':[surface(0.,rational=True),surface(4.,count=(4,4))],
 'rational_swapped':[surface(4.,count=(4,4)),surface(0.,rational=True)],
 'both_rational':[surface(0.,rational=True),surface(4.,count=(4,4),rational=True)],
 'cubic_counts':[surface(0.,degree=(3,3),count=(4,4)),surface(4.,degree=(3,3),count=(5,5))],
 'shifted_knot':[surface(0.,count=(4,4)),surface(4.,count=(4,4),knots=([0.,0.,0.,.3,1.,1.,1.],[0.,0.,0.,.7,1.,1.,1.]))],
 'curved_rows':[surface(0.),surface(4.,count=(4,4))],
}.items():
    if family=='curved_rows':
        for i,c in enumerate(sources[1]['control_points']):c['point'][2]+=.25*((i%4)**2+(i//4)**2)
    for number in (1,2):SPECS[family+'_number'+str(number)]=dict(sources=sources,method='None',number=number,sample=4,layer='CurrentLayer')


def request():
    return dict(protocol_version=1, iterations=1, operations=[dict(op='tween_surfaces_control',id='tween_srf_'+case,case=case) for case in SPECS])


def validate_request(q):
    if (not isinstance(q,dict) or set(q)-{'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):
        raise ValueError('TweenSurfaces requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='tween_surfaces_control'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or not isinstance(op['case'],str) or op['case'] not in SPECS):
            raise ValueError('invalid TweenSurfaces recipe')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System']
    if Rhino.Commands.Command.InCommand():raise ValueError('TweenSurfaces requires idle execution')
    doc,G=Rhino.RhinoDoc.ActiveDoc,Rhino.Geometry
    if list(doc.Objects):raise ValueError('TweenSurfaces requires an empty owned document')
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
        marker='Viboceros TweenSurfaces '+str(System.Guid.NewGuid())
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
                if ids[-1]==System.Guid.Empty:raise ValueError('TweenSurfaces insertion failed')
            finally:g.Dispose();a.Dispose()
        for members in [[key] for key in ids]+[ids]:groups.append(doc.Groups.Add('Viboceros TweenSurface '+str(System.Guid.NewGuid()),members))
        doc.Layers.SetCurrentLayerIndex(layers[2],True);doc.ClearUndoRecords(True)
        before=snapshot()
        macro='_TweenSurfaces '+' '.join('_SelID '+str(key) for key in ids)+' _NumberOfSurfaces='+str(spec['number'])+' _MatchMethod=_'+spec['method']
        if spec['method']=='SamplePoints':macro+=' _SampleNumber='+str(spec['sample'])
        macro+=' _OutputLayer=_'+spec['layer']+' _Enter'
        prepared=[]
        count_u=max(s['control_point_count_u']for s in spec['sources']);count_v=max(s['control_point_count_v']for s in spec['sources'])
        degree_u=max(s['degree_u']for s in spec['sources']);degree_v=max(s['degree_v']for s in spec['sources'])
        for definition in spec['sources']:
            source=host['_nurbs_surface_from_definition'](definition)
            try:
                n=source.Rebuild(degree_u,degree_v,count_u,count_v)
                if n is None:raise ValueError('surface rebuild SDK failed')
                try:prepared.append(host['_nurbs_surface_definition'](n))
                finally:n.Dispose()
            finally:source.Dispose()
        # Capture matched outputs and then public SDK rebuilding of each blend.
        matched_macro=macro.replace('_MatchMethod=_None','_MatchMethod=_Refit')
        matched=invoke(matched_macro,'TweenSurfaces')
        candidates=[]
        objects=sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber)
        for obj in objects[-spec['number']:]:
            g=obj.Geometry;s=g.Faces[0].UnderlyingSurface() if isinstance(g,G.Brep) else g
            n=s.Rebuild(degree_u,degree_v,count_u,count_v)
            if n is None:raise ValueError('matched blend rebuild failed')
            try:candidates.append(host['_nurbs_surface_definition'](n))
            finally:n.Dispose()
        for obj in list(doc.Objects):
            if obj.Id not in ids:doc.Objects.Delete(obj.Id,True)
        doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
        before=snapshot()
        result=dict(case=op['case'],spec=spec,before=before,rebuild_sdk=prepared,matched=matched,rebuilt_blends_sdk=candidates,command=invoke(macro,'TweenSurfaces'))
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
