"""Actual UnjoinEdge commands in an owned document on private Xvfb."""
import re


def validate(operation):
    required={'op','id','sources','components','finish','undo_redo'}
    if (not isinstance(operation,dict) or set(operation)-{'kind','object_preselect','pick','steps'}!=required
            or operation.get('op')!='unjoin_edge_command' or not isinstance(operation.get('id'),str)
            or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or operation.get('finish') not in ('Enter','Cancel')
            or type(operation.get('undo_redo')) is not bool or type(operation.get('object_preselect',False)) is not bool
            or operation.get('kind','edge') not in ('edge','face')
            or operation.get('pick','preselect') not in ('preselect','mouse','sequence')
            or (operation.get('kind')=='face' and operation.get('pick','preselect')!='preselect')
            or not isinstance(operation.get('sources'),list) or not 1<=len(operation['sources'])<=8
            or not isinstance(operation.get('components'),list) or len(operation['components'])>100000
            or any(not isinstance(pair,list) or len(pair)!=2 or any(type(v) is not int or v<0 for v in pair) or pair[0]>=len(operation['sources']) for pair in operation['components'])):
        raise ValueError('invalid UnjoinEdge command fixture')
    if operation.get('pick') == 'sequence':
        steps = operation.get('steps')
        if operation['components'] or operation.get('object_preselect') or not isinstance(steps, list) or not 1 <= len(steps) <= 64:
            raise ValueError('invalid UnjoinEdge input sequence')
        for position,step in enumerate(steps):
            if isinstance(step, dict) and step.get('kind') == 'key':
                if set(step) != {'kind','value'} or step.get('value') not in ('None','Undo'):
                    raise ValueError('invalid UnjoinEdge sequence key')
                if step['value']=='None' and (position!=len(steps)-1 or operation['finish']!='Cancel'):
                    raise ValueError('None terminates an UnjoinEdge sequence')
                continue
            if not isinstance(step, dict) or step.get('modifiers') not in ('plain','ctrl','shift','sub','alt'):
                raise ValueError('invalid UnjoinEdge input modifier')
            if step.get('kind') == 'click':
                pair = step.get('component')
                if (set(step) != {'kind','component','modifiers'} or not isinstance(pair, list) or len(pair) != 2
                        or any(type(v) is not int or v < 0 for v in pair) or pair[0] >= len(operation['sources'])):
                    raise ValueError('invalid UnjoinEdge sequence click')
            elif step.get('kind') == 'window':
                corners = step.get('corners')
                if (set(step) != {'kind','corners','modifiers'} or not isinstance(corners, list) or len(corners) != 2
                        or any(not isinstance(p, list) or len(p) != 3 or any(type(v) not in (int,float) or not -1e6 <= v <= 1e6 for v in p) for p in corners)):
                    raise ValueError('invalid UnjoinEdge sequence window')
            else: raise ValueError('invalid UnjoinEdge sequence step')
    elif 'steps' in operation: raise ValueError('steps require a sequence pick')
    for source in operation['sources']:
        if (not isinstance(source,dict) or set(source)!={'brep'} or not isinstance(source['brep'],dict)
                or not isinstance(source['brep'].get('artifact_path'),(str,type(u''))) or not source['brep']['artifact_path']):
            raise ValueError('UnjoinEdge requires owned B-rep sources')


def run(operation,tolerance,host):
    from join_probe import observe_command
    validate(operation)
    Rhino,System=host['Rhino'],host['System']
    if operation['undo_redo'] and Rhino.Commands.Command.InCommand(): raise ValueError('UnjoinEdge history requires idle execution')
    doc=Rhino.RhinoDoc.ActiveDoc
    settings=Rhino.DocObjects.ObjectEnumeratorSettings(); settings.NormalObjects=settings.HiddenObjects=settings.LockedObjects=True
    def objects(): return list(doc.Objects.GetObjectList(settings))
    original=set(obj.Id for obj in objects()); old_selection=[obj.Id for obj in objects() if obj.IsSelected(False)]
    ids,groups,owned=[],[],[]
    def geometry(g):
        if not isinstance(g,Rhino.Geometry.Brep): raise ValueError('unexpected separated geometry type')
        return dict(type='brep',definition=host['_interchange_brep_record'](g,include_samples=False),untrimmed=[bool(face.IsSurface) for face in g.Faces])
    def snapshot():
        result=[]
        for obj in sorted((obj for obj in objects() if obj.Id not in original),key=lambda obj: obj.RuntimeSerialNumber):
            attrs=obj.Attributes
            result.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,selected=bool(obj.IsSelected(False)),name=attrs.Name,
                color=[int(attrs.ObjectColor.R),int(attrs.ObjectColor.G),int(attrs.ObjectColor.B)],color_source=str(attrs.ColorSource),
                current_layer=attrs.LayerIndex==doc.Layers.CurrentLayerIndex,groups=sorted(groups.index(g) for g in (attrs.GetGroupList() or []) if g in groups),geometry=geometry(obj.Geometry)))
        return result
    def components():
        kinds={'BrepEdge':'edge','BrepFace':'face'};result=[]
        for source,key in enumerate(ids):
            obj=doc.Objects.FindId(key)
            if obj is None: continue
            for component in (obj.GetSelectedSubObjects() or []):
                result.append([source,kinds[str(component.ComponentIndexType)],int(component.Index)])
        return sorted(result)
    try:
        doc.Objects.UnselectAll();constructed=[]
        for index,source in enumerate(operation['sources']):
            path=source['brep']['artifact_path']
            if path.startswith('/'): path='Z:'+path.replace('/','\\')
            model=Rhino.FileIO.File3dm.Read(path)
            if model is None: raise ValueError('cannot read UnjoinEdge source')
            try:
                entries=list(model.Objects)
                if len(entries)!=1: raise ValueError('UnjoinEdge needs one object per artifact')
                g=entries[0].Geometry.Duplicate()
            finally: model.Dispose()
            owned.append(g)
            if not isinstance(g,Rhino.Geometry.Brep) or not g.IsValid: raise ValueError('invalid UnjoinEdge source')
            constructed.append(geometry(g));attrs=Rhino.DocObjects.ObjectAttributes()
            try:
                attrs.Name='source-%d'%index; attrs.ObjectColor=System.Drawing.Color.FromArgb(10+index,30,50);attrs.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
                key=doc.Objects.AddBrep(g,attrs,None,False,False)
            finally: attrs.Dispose()
            if key==System.Guid.Empty: raise ValueError('source insertion failed')
            ids.append(key)
        for key in ids:
            group=doc.Groups.Add('Viboceros unjoin '+str(System.Guid.NewGuid()),[key])
            if group<0: raise ValueError('source grouping failed')
            groups.append(group)
        kind=Rhino.Geometry.ComponentIndexType.BrepFace if operation.get('kind')=='face' else Rhino.Geometry.ComponentIndexType.BrepEdge
        points=[]
        for source,index in operation['components']:
            obj=doc.Objects.FindId(ids[source]); count=obj.Geometry.Faces.Count if operation.get('kind')=='face' else obj.Geometry.Edges.Count
            if index>=count: raise ValueError('component outside source')
            if operation.get('pick','preselect')=='preselect':
                if not obj.SelectSubObject(Rhino.Geometry.ComponentIndex(kind,index),True,True,False): raise ValueError('component preselection failed')
            else:
                edge=obj.Geometry.Edges[index]; points.append(edge.PointAt(edge.Domain.ParameterAt(.375)))
        if operation.get('object_preselect'):
            for key in ids: doc.Objects.Select(key)
        before=snapshot(); selection_before=components();marker='Viboceros UnjoinEdge '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        Rhino.RhinoApp.WriteLine('Component enumeration: '+str([[int(component.Index) for component in (doc.Objects.FindId(key).GetSelectedSubObjects() or [])] for key in ids]))
        trace=[]
        if operation.get('pick')=='sequence':
            from unjoin_edge_input import drive
            points=[]
            for step in operation['steps']:
                if step['kind']=='click':
                    source,index=step['component'];g=doc.Objects.FindId(ids[source]).Geometry
                    if index>=g.Edges.Count: raise ValueError('component outside source')
                    edge=g.Edges[index];points.append([edge.PointAt(edge.Domain.ParameterAt(.375))])
                elif step['kind']=='window': points.append([Rhino.Geometry.Point3d(*point) for point in step['corners']])
                else: points.append([])
            Rhino.RhinoApp.RunScript('_SetView _World _Top',False);Rhino.RhinoApp.RunScript('_Zoom _Extents',False)
            for unused in range(3): Rhino.RhinoApp.RunScript('_Zoom _Out',False)
            doc.Views.Redraw()
            action=lambda: drive(operation,points,host,components,trace)
        elif operation.get('pick')=='mouse':
            from untrim_holes_probe import drive
            Rhino.RhinoApp.RunScript('_SetView _World _Top',False);Rhino.RhinoApp.RunScript('_Zoom _Extents',False);doc.Views.Redraw()
            action=lambda: drive(dict(operation,pick='mouse'),points,host,'UnjoinEdge')
        else: action=lambda: Rhino.RhinoApp.RunScript('_UnjoinEdge _'+operation['finish'],True)
        succeeded,after,events=observe_command(Rhino.Commands.Command,'UnjoinEdge',action,snapshot,lambda: [],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)
        if len(history)!=2: raise ValueError('history marker missing')
        for event in events: event.pop('objects',None)
        result=dict(constructed=constructed,before=before,after=after,succeeded=succeeded,events=events,history=history[1].strip(),component_selection=dict(before=selection_before,after=components()))
        if operation.get('pick')=='sequence': result['selection_steps']=trace
        if operation['undo_redo']:
            result['history_tested']=[{k:v for k,v in obj.items() if k!='selected'} for obj in before]!=[{k:v for k,v in obj.items() if k!='selected'} for obj in after]
            if result['history_tested']:
                for command in ('Undo','Redo'):
                    ok,state,events=observe_command(Rhino.Commands.Command,command,lambda command=command: Rhino.RhinoApp.RunScript('_'+command,True),snapshot,lambda: [],True)
                    if not ok: raise ValueError('UnjoinEdge history failed')
                    for event in events: event.pop('objects',None)
                    result[command.lower()]=snapshot();result[command.lower()+'_events']=events;result['component_selection'][command.lower()]=components()
        return result,0
    finally:
        Rhino.RhinoApp.RunScript('!',False)
        for obj in objects():
            if obj.Id not in original: doc.Objects.Delete(obj.Id,True)
        for group in groups: doc.Groups.Delete(group)
        for g in owned: g.Dispose()
        for key in old_selection: doc.Objects.Select(key)
