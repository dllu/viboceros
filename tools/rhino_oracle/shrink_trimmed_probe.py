"""Whole-object shrink commands on independent B-reps in private Xvfb."""
import re

COMMANDS={'shrink_trimmed_srf_command':'ShrinkTrimmedSrf',
          'shrink_trimmed_srf_to_edge_command':'ShrinkTrimmedSrfToEdge'}


def validate(operation):
    if (not isinstance(operation,dict)
            or set(operation)!={'op','id','sources','preselect','order','undo_redo'}
            or not isinstance(operation.get('op'),str) or operation.get('op') not in COMMANDS
            or not isinstance(operation.get('id'),str)
            or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or any(type(operation.get(key)) is not bool for key in ('preselect','undo_redo'))
            or not isinstance(operation.get('sources'),list) or not 1<=len(operation['sources'])<=8
            or not isinstance(operation.get('order'),list)
            or any(type(i) is not int for i in operation['order'])
            or sorted(operation['order'])!=list(range(len(operation['sources'])))):
        raise ValueError('invalid shrink command fixture')
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
        if operation['preselect']:
            for index in operation['order']:fixture.doc.Objects.Select(fixture.ids[index])
            suffix=' _Enter'
        else:suffix=' '+' '.join('_SelID %s'%fixture.ids[i] for i in operation['order'])+' _Enter'
        before=fixture.snapshot();marker='Viboceros '+command+' '+str(host['System'].Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        ok,after,events=observe_command(Rhino.Commands.Command,command,
            lambda:Rhino.RhinoApp.RunScript('_'+command+suffix,True),fixture.snapshot,lambda:[],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)
        if len(history)!=2:raise ValueError('shrink history marker missing')
        for event in events:event.pop('objects',None)
        result=dict(constructed=constructed,before=before,after=after,succeeded=ok,events=events,history=history[1].strip())
        if operation['undo_redo']:
            result['history_tested']=[{k:v for k,v in obj.items() if k!='selected'} for obj in before]!=[{k:v for k,v in obj.items() if k!='selected'} for obj in after]
            if result['history_tested']:
                for name in ('Undo','Redo'):
                    ok,state,events=observe_command(Rhino.Commands.Command,name,
                        lambda name=name:Rhino.RhinoApp.RunScript('_'+name,True),fixture.snapshot,lambda:[],True)
                    if not ok:raise ValueError('shrink external history failed')
                    for event in events:event.pop('objects',None)
                    result[name.lower()]=fixture.snapshot();result[name.lower()+'_events']=events
        return result,0
