"""Native ExtractSrf from independently exported, owned face targets."""
import re


def validate(operation):
    required={'op','id','sources','components','copy','output_current','source_layer','undo_redo'}
    if (not isinstance(operation,dict) or set(operation)!=required
            or operation.get('op')!='extract_srf_command'
            or not isinstance(operation.get('id'),str)
            or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or any(type(operation.get(key)) is not bool for key in ('copy','output_current','source_layer','undo_redo'))
            or not isinstance(operation.get('sources'),list) or not 1<=len(operation['sources'])<=8
            or not isinstance(operation.get('components'),list) or not 1<=len(operation['components'])<=64):
        raise ValueError('invalid ExtractSrf fixture')
    for pair in operation['components']:
        if (not isinstance(pair,list) or len(pair)!=2 or any(type(i) is not int or i<0 for i in pair)
                or pair[0]>=len(operation['sources'])):
            raise ValueError('invalid ExtractSrf face target')
    for source in operation['sources']:
        if (not isinstance(source,dict) or set(source)!={'brep'} or not isinstance(source['brep'],dict)
                or not isinstance(source['brep'].get('artifact_path'),(str,type(u'')))
                or not source['brep']['artifact_path']):
            raise ValueError('ExtractSrf requires independent owned B-reps')


def run(operation,tolerance,host):
    from owned_brep_command import OwnedBrepCommand
    from join_probe import observe_command
    validate(operation);Rhino,System=host['Rhino'],host['System'];doc=Rhino.RhinoDoc.ActiveDoc
    if operation['undo_redo'] and Rhino.Commands.Command.InCommand():
        raise ValueError('ExtractSrf history requires idle execution')
    layer=None
    try:
        with OwnedBrepCommand(host) as fixture:
            constructed=fixture.setup(operation['sources'])
            if operation['source_layer']:
                layer=doc.Layers.Add('Viboceros extraction '+str(System.Guid.NewGuid()),System.Drawing.Color.Black)
                if layer<0:raise ValueError('ExtractSrf source layer creation failed')
                for key in fixture.ids:
                    attributes=doc.Objects.FindId(key).Attributes.Duplicate()
                    try:
                        attributes.LayerIndex=layer
                        if not doc.Objects.ModifyAttributes(key,attributes,True):raise ValueError('ExtractSrf source layer assignment failed')
                    finally:attributes.Dispose()
            for source,index in operation['components']:
                obj=doc.Objects.FindId(fixture.ids[source])
                if index>=obj.Geometry.Faces.Count:raise ValueError('ExtractSrf face outside source')
                component=Rhino.Geometry.ComponentIndex(Rhino.Geometry.ComponentIndexType.BrepFace,index)
                if not obj.SelectSubObject(component,True,True,False):raise ValueError('ExtractSrf face preselection failed')
            before=fixture.snapshot();components=fixture.components()
            marker='Viboceros ExtractSrf '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
            script='_ExtractSrf _Copy=_%s _OutputLayer=_%s _Enter' % (
                'Yes' if operation['copy'] else 'No','Current' if operation['output_current'] else 'Input')
            ok,after,events=observe_command(Rhino.Commands.Command,'ExtractSrf',
                lambda:Rhino.RhinoApp.RunScript(script,True),fixture.snapshot,lambda:[],True)
            history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)
            if len(history)!=2:raise ValueError('ExtractSrf history marker missing')
            for event in events:event.pop('objects',None)
            result=dict(constructed=constructed,before=before,after=after,succeeded=ok,events=events,
                history=history[1].strip(),component_selection=dict(before=components,after=fixture.components()))
            if operation['undo_redo']:
                for name in ('Undo','Redo'):
                    ok,state,events=observe_command(Rhino.Commands.Command,name,
                        lambda name=name:Rhino.RhinoApp.RunScript('_'+name,True),fixture.snapshot,lambda:[],True)
                    if not ok:raise ValueError('ExtractSrf external history failed')
                    for event in events:event.pop('objects',None)
                    result[name.lower()]=state;result[name.lower()+'_events']=events
                    result['component_selection'][name.lower()]=fixture.components()
            return result,0
    finally:
        if layer is not None and layer>=0 and not doc.Layers.Delete(layer,True):
            raise ValueError('ExtractSrf source layer cleanup failed')
