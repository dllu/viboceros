"""Observe command Copy defaults in an owned document and private settings scheme."""
import re


def validate(operation):
    if (not isinstance(operation,dict) or set(operation)!={'op','id','sources','steps'}
            or operation['op']!='copy_options_command'
            or not isinstance(operation['id'],str)
            or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or not isinstance(operation['sources'],list) or len(operation['sources'])!=1
            or not isinstance(operation['steps'],list) or not 1<=len(operation['steps'])<=128):
        raise ValueError('invalid Copy options workflow')
    source=operation['sources'][0]
    if (not isinstance(source,dict) or set(source)!={'brep'} or not isinstance(source['brep'],dict)
            or not isinstance(source['brep'].get('artifact_path'),str) or not source['brep']['artifact_path']):
        raise ValueError('Copy workflow requires an independent owned B-rep')
    for step in operation['steps']:
        if (not isinstance(step,dict) or set(step)!={'command','copy','finish'}
                or step['command'] not in ('RememberCopyOptions','ExtractSrf','Rotate','Scale','Mirror')
                or (step['copy'] is not None and type(step['copy']) is not bool)
                or step['finish'] not in ('Complete','Cancel')
                or (step['command']=='RememberCopyOptions' and step['finish']=='Cancel' and step['copy'] is not None)):
            raise ValueError('invalid Copy workflow step')


def run(operation,host):
    from owned_brep_command import OwnedBrepCommand
    from join_probe import observe_command
    validate(operation);Rhino,System=host['Rhino'],host['System'];doc=Rhino.RhinoDoc.ActiveDoc
    records=[]
    doc.Views.ActiveView.ActiveViewport.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
    for step in operation['steps']:
        name,choice,finish=step['command'],step['copy'],step['finish']
        with OwnedBrepCommand(host) as fixture:
            key=None;query=None
            if name=='RememberCopyOptions':
                suffix='_Cancel' if finish=='Cancel' else '_Enter' if choice is None else '_Yes' if choice else '_No'
                macro='_RememberCopyOptions '+suffix
            else:
                if name=='ExtractSrf':
                    fixture.setup(operation['sources']);key=fixture.ids[0]
                    if finish!='Cancel':
                        component=Rhino.Geometry.ComponentIndex(Rhino.Geometry.ComponentIndexType.BrepFace,0)
                        if not doc.Objects.FindId(key).SelectSubObject(component,True,True,False):
                            raise ValueError('Copy workflow face preselection failed')
                    base='_ExtractSrf';tail='_Enter'
                else:
                    doc.Objects.UnselectAll();key=doc.Objects.AddPoint(Rhino.Geometry.Point3d(2,3,4))
                    if key==System.Guid.Empty:raise ValueError('Copy workflow point insertion failed')
                    fixture.ids.append(key);doc.Objects.Select(key)
                    base='_'+name+' w0,0,0';tail={'Rotate':'90','Scale':'2','Mirror':'w0,1,0'}[name]
                    # This cancelled, unedited query reads the real default so
                    # a repeated-copy command receives Enter only when needed.
                    # No flag changes occur between the query and measured run.
                    marker='Viboceros Copy query '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
                    Rhino.RhinoApp.RunScript(base+' _Cancel',True)
                    query=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
                    matches=re.findall(r'Copy=(Yes|No)',query)
                    if not matches:raise ValueError('Copy workflow default query missing')
                    default=matches[0]=='Yes';doc.Objects.Select(key)
                option=' _Copy=_'+('Yes' if choice else 'No') if choice is not None else ''
                macro=base+option+' '+('_Cancel' if finish=='Cancel' else tail)
                if name in ('Rotate','Scale') and finish=='Complete' and (choice if choice is not None else default):
                    macro+=' _Enter'
            def snapshot():
                result=[]
                for obj in sorted((obj for obj in fixture.objects() if obj.Id not in fixture.original),key=lambda obj:obj.RuntimeSerialNumber):
                    if isinstance(obj.Geometry,Rhino.Geometry.Brep):geometry=fixture.geometry(obj.Geometry)
                    elif isinstance(obj.Geometry,Rhino.Geometry.Point):
                        geometry=dict(type='point',point=host['_xyz'](obj.Geometry.Location))
                    else:raise ValueError('unexpected Copy workflow geometry')
                    result.append(dict(source=obj.Id==key,selected=bool(obj.IsSelected(False)),geometry=geometry))
                return result
            before=snapshot();marker='Viboceros Copy run '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
            host['_record_progress']('copy workflow '+macro)
            succeeded,after,events=observe_command(Rhino.Commands.Command,name,
                lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
            history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
            if finish=='Cancel' and after!=before:
                raise ValueError('Copy workflow cancellation edited geometry')
            records.append(dict(command=name,copy=choice,finish=finish,succeeded=succeeded,
                before=before,after=after,history=history,query=query,events=events))
    return dict(records=records),0
