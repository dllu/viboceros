"""Actual shrink commands with owned whole-object and face selection."""
import re

COMMANDS={'shrink_trimmed_srf_command':'ShrinkTrimmedSrf',
          'shrink_trimmed_srf_to_edge_command':'ShrinkTrimmedSrfToEdge'}


def validate(operation):
    required={'op','id','sources','preselect','order','undo_redo'}
    optional={'components','objects','steps','pick','finish'}
    if (not isinstance(operation,dict) or set(operation)-optional!=required
            or not isinstance(operation.get('op'),str) or operation.get('op') not in COMMANDS
            or not isinstance(operation.get('id'),str)
            or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or any(type(operation.get(key)) is not bool for key in ('preselect','undo_redo'))
            or not isinstance(operation.get('sources'),list) or not 1<=len(operation['sources'])<=8
            or not isinstance(operation.get('order'),list)
            or any(type(i) is not int for i in operation['order'])
            or sorted(operation['order'])!=list(range(len(operation['sources'])))):
        raise ValueError('invalid shrink command fixture')
    def pair(pair):
        return (isinstance(pair,list) and len(pair)==2 and all(type(v) is int and v>=0 for v in pair)
                and pair[0]<len(operation['sources']))
    if 'components' in operation:
        components=operation['components'];objects=operation.get('objects',[])
        if (not isinstance(components,list) or len(components)>64 or not all(pair(p) for p in components)
                or not isinstance(objects,list) or any(type(i) is not int or not 0<=i<len(operation['sources']) for i in objects)
                or len(set(objects))!=len(objects)):
            raise ValueError('invalid shrink component targets')
        if operation.get('pick','preselect')=='preselect':
            if not operation['preselect'] or not components or 'steps' in operation:
                raise ValueError('shrink face preselection requires components')
        elif operation.get('pick')=='sequence':
            steps=operation.get('steps')
            if operation['preselect'] or components or objects or not isinstance(steps,list) or not 1<=len(steps)<=64:
                raise ValueError('invalid shrink input sequence')
            for step in steps:
                if (not isinstance(step,dict) or set(step)!={'kind','component','modifiers'} or step['kind']!='click'
                        or step['modifiers'] not in ('sub','plain','ctrl','shift') or not pair(step['component'])):
                    raise ValueError('invalid shrink face input')
        else:raise ValueError('invalid shrink picking mode')
        if operation.get('finish','Enter') not in ('Enter','Cancel'):raise ValueError('invalid shrink finish')
    elif set(operation)&optional:raise ValueError('component fields require face targets')
    for source in operation['sources']:
        if (not isinstance(source,dict) or set(source)!={'brep'} or not isinstance(source['brep'],dict)
                or not isinstance(source['brep'].get('artifact_path'),(str,type(u'')))
                or not source['brep']['artifact_path']):
            raise ValueError('shrink commands require independent owned B-reps')


def run(operation,tolerance,host):
    from owned_brep_command import OwnedBrepCommand
    from join_probe import observe_command
    validate(operation);Rhino=host['Rhino'];command=COMMANDS[operation['op']]
    if operation['undo_redo'] and Rhino.Commands.Command.InCommand():
        raise ValueError('shrink history requires idle execution')
    with OwnedBrepCommand(host) as fixture:
        constructed=fixture.setup(operation['sources'])
        if 'components' in operation:
            for source in operation.get('objects',[]):fixture.doc.Objects.Select(fixture.ids[source])
            for source,index in operation['components']:
                obj=fixture.doc.Objects.FindId(fixture.ids[source])
                if index>=obj.Geometry.Faces.Count:raise ValueError('shrink face outside source')
                component=Rhino.Geometry.ComponentIndex(Rhino.Geometry.ComponentIndexType.BrepFace,index)
                if not obj.SelectSubObject(component,True,True,False):raise ValueError('shrink face preselection failed')
            suffix=' _'+operation.get('finish','Enter')
        elif operation['preselect']:
            for index in operation['order']:fixture.doc.Objects.Select(fixture.ids[index])
            suffix=' _Enter'
        else:suffix=' '+' '.join('_SelID %s'%fixture.ids[i] for i in operation['order'])+' _Enter'
        steps=operation.get('steps');points=[];trace=[]
        if steps:
            for step in steps:
                source,index=step['component'];obj=fixture.doc.Objects.FindId(fixture.ids[source])
                if index>=obj.Geometry.Faces.Count:raise ValueError('shrink face outside source')
                face=obj.Geometry.Faces[index]
                uv=next(((face.Domain(0).ParameterAt(u),face.Domain(1).ParameterAt(v))
                    for u in (.3,.5,.7) for v in (.3,.5,.7)
                    if str(face.IsPointOnFace(face.Domain(0).ParameterAt(u),face.Domain(1).ParameterAt(v)))=='Interior'),None)
                if uv is None:raise ValueError('shrink face has no certified interior pick')
                points.append([face.PointAt(*uv)])
            Rhino.RhinoApp.RunScript('_SetView _World _Top',False);Rhino.RhinoApp.RunScript('_Zoom _Extents',False)
            mode=Rhino.Display.DisplayModeDescription.FindByName('Shaded')
            if mode is None:raise ValueError('shaded mode unavailable for shrink face picking')
            fixture.doc.Views.ActiveView.ActiveViewport.DisplayMode=mode;fixture.doc.Views.Redraw()
        before=fixture.snapshot();components=fixture.components();marker='Viboceros '+command+' '+str(host['System'].Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        if steps:
            from shrink_face_input import drive
            action=lambda:drive(operation,points,host,fixture.components,trace,command)
        else:action=lambda:Rhino.RhinoApp.RunScript('_'+command+suffix,True)
        ok,after,events=observe_command(Rhino.Commands.Command,command,action,fixture.snapshot,lambda:[],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)
        if len(history)!=2:raise ValueError('shrink history marker missing')
        for event in events:event.pop('objects',None)
        result=dict(constructed=constructed,before=before,after=after,succeeded=ok,events=events,history=history[1].strip())
        if 'components' in operation:result['component_selection']=dict(before=components,after=fixture.components())
        if steps:result['selection_steps']=trace
        if operation['undo_redo']:
            result['history_tested']=[{k:v for k,v in obj.items() if k!='selected'} for obj in before]!=[{k:v for k,v in obj.items() if k!='selected'} for obj in after]
            if result['history_tested']:
                for name in ('Undo','Redo'):
                    ok,state,events=observe_command(Rhino.Commands.Command,name,
                        lambda name=name:Rhino.RhinoApp.RunScript('_'+name,True),fixture.snapshot,lambda:[],True)
                    if not ok:raise ValueError('shrink external history failed')
                    for event in events:event.pop('objects',None)
                    result[name.lower()]=fixture.snapshot();result[name.lower()+'_events']=events
                    if 'components' in operation:result['component_selection'][name.lower()]=fixture.components()
        return result,0
