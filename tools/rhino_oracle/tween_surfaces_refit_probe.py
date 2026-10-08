"""Closed, owned public TweenSurfaces command recipes and complete surface data."""
import re


def surface(z, degree=1, count=2, rational=False, shifted=False):
    knots = [0.]*(degree+1) + ([.5] if count > degree+1 else []) + [1.]*(degree+1)
    if shifted:
        knots_u, knots_v = [2.+3.*x for x in knots], [-4.+6.*x for x in knots]
    else:
        knots_u = knots_v = knots
    return dict(degree_u=degree, degree_v=degree, control_point_count_u=count, control_point_count_v=count,
                knots_u=knots_u, knots_v=knots_v,
                control_points=[dict(point=[4.*u/(count-1), 6.*v/(count-1), z+(u*v if degree>1 else 0.)],
                                     weight=1.+.25*(u+v) if rational else 1.)
                                for v in range(count) for u in range(count)])


SPECS = {}
for family, sources in {
    'planes':[surface(0.),surface(4.)],
    'warped':[surface(0.,2,3),surface(4.,2,3)],
    'rational':[surface(0.,2,3,True),surface(4.,2,3)],
    'rational_swapped':[surface(4.,2,3),surface(0.,2,3,True)],
    'both_rational':[surface(0.,2,3,True),surface(4.,2,3,True)],
    'counts':[surface(0.,2,3),surface(4.,2,4)],
    'counts_swapped':[surface(4.,2,4),surface(0.,2,3)],
    'degrees':[surface(0.),surface(4.,2,3)],
    'degrees_swapped':[surface(4.,2,3),surface(0.)],
    'domains':[surface(0.,2,3),surface(4.,2,3,shifted=True)],
    'rational_counts':[surface(0.,2,3,True),surface(4.,2,4)],
}.items():
    for number in (1,3):
        SPECS[family+'_number'+str(number)]=dict(sources=sources,method='Refit',number=number,sample=4,layer='CurrentLayer')
for layer in ('StartSrf','EndSrf'):
    SPECS['layer_'+layer.lower()]=dict(sources=[surface(0.,2,3),surface(4.,2,3)],method='Refit',number=2,sample=4,layer=layer)


def request():
    return dict(protocol_version=1, iterations=1, operations=[dict(op='tween_surfaces_refit',id='tween_srf_'+case,case=case) for case in SPECS])


def validate_request(q):
    if (not isinstance(q,dict) or set(q)-{'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):
        raise ValueError('TweenSurfaces requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='tween_surfaces_refit'
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
        initialization=invoke(macro,'TweenSurfaces')
        for obj in list(doc.Objects):
            if obj.Id not in ids:doc.Objects.Delete(obj.Id,True)
        doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
        before=snapshot()
        bare='_TweenSurfaces '+' '.join('_SelID '+str(key) for key in ids)+' _Enter'
        result=dict(case=op['case'],spec=spec,before=before,initialization=initialization,command=invoke(bare,'TweenSurfaces'))
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
