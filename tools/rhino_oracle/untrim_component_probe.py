"""Actual Untrim edge picks on independent owned B-reps, in private Xvfb."""
import re


def validate(operation):
    fields={'op','id','sources','components','all_similar','keep_trim_objects','pick','finish','undo_redo'}
    if (not isinstance(operation,dict) or set(operation)-{'view','undo_after'}!=fields or operation.get('op')!='untrim_command'
            or not isinstance(operation.get('id'),str) or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or any(type(operation.get(name)) is not bool for name in ('all_similar','keep_trim_objects','undo_redo'))
            or operation.get('pick') not in ('preselect','mouse') or operation.get('finish') not in ('Enter','Cancel')
            or operation.get('view','top') not in ('top','oblique') or ('view' in operation and operation.get('pick')!='mouse')
            or not isinstance(operation.get('sources'),list) or not 1<=len(operation['sources'])<=8
            or not isinstance(operation.get('components'),list) or len(operation['components'])>64
            or any(not isinstance(pair,list) or len(pair)!=2 or any(type(v) is not int or v<0 for v in pair)
                or pair[0]>=len(operation['sources']) for pair in operation['components'])):
        raise ValueError('invalid Untrim component fixture')
    for source in operation['sources']:
        if (not isinstance(source,dict) or set(source)!={'brep'} or not isinstance(source['brep'],dict)
                or not isinstance(source['brep'].get('artifact_path'),(str,type(u''))) or not source['brep']['artifact_path']):
            raise ValueError('Untrim requires independent owned B-rep sources')
    undo=operation.get('undo_after',[])
    if (not isinstance(undo,list) or any(type(i) is not int or not 1<=i<=len(operation['components']) for i in undo)
            or any(a>=b for a,b in zip(undo,undo[1:])) or undo and operation['pick']!='mouse'):
        raise ValueError('invalid Untrim local Undo checkpoints')


def run(operation,tolerance,host):
    from owned_brep_command import OwnedBrepCommand
    from join_probe import observe_command
    validate(operation);Rhino=host['Rhino']
    if operation['undo_redo'] and Rhino.Commands.Command.InCommand():raise ValueError('Untrim history requires idle execution')
    with OwnedBrepCommand(host) as fixture:
        fixture.doc.Objects.UnselectAll()
        options=' _AllSimilar=%s _KeepTrimObjects=%s'%('Yes' if operation['all_similar'] else 'No','Yes' if operation['keep_trim_objects'] else 'No')
        Rhino.RhinoApp.RunScript('_Untrim'+options+' _Cancel',False)
        constructed=fixture.setup(operation['sources']);points=[]
        for source,index in operation['components']:
            obj=fixture.doc.Objects.FindId(fixture.ids[source])
            if index>=obj.Geometry.Edges.Count:raise ValueError('Untrim edge outside source')
            if operation['pick']=='preselect':
                component=Rhino.Geometry.ComponentIndex(Rhino.Geometry.ComponentIndexType.BrepEdge,index)
                if not obj.SelectSubObject(component,True,True,False):raise ValueError('Untrim edge preselection failed')
            else:
                edge=obj.Geometry.Edges[index];points.append(edge.PointAt(edge.Domain.ParameterAt(.375)))
        if operation['pick']=='mouse':
            Rhino.RhinoApp.RunScript('_SetView _World _Top',False)
            if operation.get('view')=='oblique':
                viewport=fixture.doc.Views.ActiveView.ActiveViewport
                viewport.SetCameraDirection(Rhino.Geometry.Vector3d(-1.,1.,-1.),True)
                viewport.CameraUp=Rhino.Geometry.Vector3d(0.,0.,1.)
            Rhino.RhinoApp.RunScript('_Zoom _Extents',False);fixture.doc.Views.Redraw()
            if points:
                # Check the original target before entering the native command's
                # GetObject event loop. Later edits can renumber source edges.
                view=fixture.doc.Views.ActiveView;viewport=view.ActiveViewport
                pixel=viewport.WorldToClient(points[0]);x,y=int(pixel.X),int(pixel.Y)
                if not 1<=x<viewport.Size.Width-1 or not 1<=y<viewport.Size.Height-1:
                    raise ValueError('Untrim first edge pick outside owned viewport')
                source,index=operation['components'][0]
                fixture.verify_edge_pick(source,index,view,viewport,x,y)
        before=fixture.snapshot();selected=fixture.components();marker='Viboceros Untrim '+str(host['System'].Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        states=[];undo_states=[]
        if operation['pick']=='mouse':
            from component_command_input import drive
            action=lambda:drive(operation,points,host,'Untrim',fixture.snapshot,states,undo_states)
        else:action=lambda:Rhino.RhinoApp.RunScript('_Untrim _'+operation['finish'],True)
        succeeded,after,events=observe_command(Rhino.Commands.Command,'Untrim',action,fixture.snapshot,lambda:[],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)
        if len(history)!=2:raise ValueError('Untrim history marker missing')
        for event in events:event.pop('objects',None)
        result=dict(constructed=constructed,before=before,after=after,succeeded=succeeded,events=events,history=history[1].strip(),component_selection=dict(before=selected,after=fixture.components()))
        if operation['pick']=='mouse':result['input_states']=states
        if operation.get('undo_after'):result['undo_states']=undo_states
        if operation['undo_redo']:
            result['history_tested']=[{k:v for k,v in obj.items() if k!='selected'} for obj in before]!=[{k:v for k,v in obj.items() if k!='selected'} for obj in after]
            if result['history_tested']:
                for command in ('Undo','Redo'):
                    ok,state,events=observe_command(Rhino.Commands.Command,command,lambda command=command:Rhino.RhinoApp.RunScript('_'+command,True),fixture.snapshot,lambda:[],True)
                    if not ok:raise ValueError('Untrim external history failed')
                    for event in events:event.pop('objects',None)
                    result[command.lower()]=fixture.snapshot();result[command.lower()+'_events']=events;result['component_selection'][command.lower()]=fixture.components()
        return result,0
