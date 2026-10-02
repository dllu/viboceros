"""Native ExtractSrf from independently exported, owned face targets."""
import re


def validate(operation):
    required={'op','id','sources','components','copy','output_current','source_layer','undo_redo'}
    optional={'pick','steps','finish','view','display'}
    if (not isinstance(operation,dict) or set(operation)-optional!=required
            or operation.get('op')!='extract_srf_command'
            or not isinstance(operation.get('id'),str)
            or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z',operation['id']) is None
            or any(type(operation.get(key)) is not bool for key in ('copy','output_current','source_layer','undo_redo'))
            or not isinstance(operation.get('sources'),list) or not 1<=len(operation['sources'])<=8
            or not isinstance(operation.get('components'),list) or len(operation['components'])>64):
        raise ValueError('invalid ExtractSrf fixture')
    for pair in operation['components']:
        if (not isinstance(pair,list) or len(pair)!=2 or any(type(i) is not int or i<0 for i in pair)
                or pair[0]>=len(operation['sources'])):
            raise ValueError('invalid ExtractSrf face target')
    if operation.get('pick','preselect')=='preselect':
        if not operation['components'] or set(operation)&optional:
            raise ValueError('ExtractSrf preselection requires face targets only')
    elif operation.get('pick')=='sequence':
        if (operation.get('view','Top') not in ('Top','Bottom','Front','Back','Left','Right')
                or operation.get('display','Shaded') not in ('Shaded','Ghosted','Wireframe')):
            raise ValueError('invalid ExtractSrf viewport')
        steps=operation.get('steps')
        if (operation['components'] or not isinstance(steps,list) or not 1<=len(steps)<=64
                or operation.get('finish') not in ('Enter','Cancel')):
            raise ValueError('invalid ExtractSrf input sequence')
        for index,step in enumerate(steps):
            if not isinstance(step,dict):raise ValueError('invalid ExtractSrf mouse target')
            if step.get('kind')=='click':
                if (set(step)-{'fraction'}!={'kind','component','modifiers'} or step['modifiers'] not in ('plain','ctrl','shift','sub')
                        or not isinstance(step['component'],list) or len(step['component'])!=2
                        or any(type(i) is not int or i<0 for i in step['component'])
                        or step['component'][0]>=len(operation['sources'])):
                    raise ValueError('invalid ExtractSrf mouse target')
                if 'fraction' in step:
                    fraction=step['fraction']
                    if (not isinstance(fraction,list) or len(fraction)!=2
                            or any(type(v) not in (int,float) or not 0<v<1 for v in fraction)):
                        raise ValueError('invalid ExtractSrf parameter fraction')
            elif step.get('kind')=='window':
                corners=step.get('corners')
                if (set(step)!={'kind','corners','modifiers'} or step['modifiers'] not in ('plain','ctrl','shift','sub')
                        or not isinstance(corners,list) or len(corners)!=2
                        or any(not isinstance(p,list) or len(p)!=3 or any(type(v) not in (int,float) or not -1e6<=v<=1e6 for v in p) for p in corners)):
                    raise ValueError('invalid ExtractSrf rectangle')
            elif step.get('kind')=='key':
                if (set(step)!={'kind','value'} or step['value'] not in ('None','Copy=Yes','Copy=No','OutputLayer=Current','OutputLayer=Input')
                        or (step['value']=='None' and index!=len(steps)-1)):
                    raise ValueError('invalid ExtractSrf option input')
            else:
                raise ValueError('invalid ExtractSrf mouse target')
    else:raise ValueError('invalid ExtractSrf input mode')
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
            steps=operation.get('steps');points=[];trace=[];completion=[]
            if steps:
                for step in steps:
                    if step['kind']=='window':
                        points.append([Rhino.Geometry.Point3d(*point) for point in step['corners']]);continue
                    if step['kind']=='key':points.append([]);continue
                    source,index=step['component'];obj=doc.Objects.FindId(fixture.ids[source])
                    if index>=obj.Geometry.Faces.Count:raise ValueError('ExtractSrf mouse face outside source')
                    face=obj.Geometry.Faces[index]
                    fractions=[step['fraction']] if 'fraction' in step else [(u,v) for u in (.3,.5,.7) for v in (.3,.5,.7)]
                    uv=next(((face.Domain(0).ParameterAt(u),face.Domain(1).ParameterAt(v))
                        for u,v in fractions
                        if str(face.IsPointOnFace(face.Domain(0).ParameterAt(u),face.Domain(1).ParameterAt(v)))=='Interior'),None)
                    if uv is None:raise ValueError('ExtractSrf face has no certified interior pick')
                    points.append([face.PointAt(*uv)])
                Rhino.RhinoApp.RunScript('_SetView _World _'+operation.get('view','Top'),False);Rhino.RhinoApp.RunScript('_Zoom _Extents',False)
                mode=Rhino.Display.DisplayModeDescription.FindByName(operation.get('display','Shaded'))
                if mode is None:raise ValueError('display mode unavailable for extraction')
                doc.Views.ActiveView.ActiveViewport.DisplayMode=mode;doc.Views.Redraw()
                view=doc.Views.ActiveView;viewport=view.ActiveViewport
                for step,point in zip(steps,points):
                    if step['kind']!='click':continue
                    pixel=viewport.WorldToClient(point[0]);source,index=step['component']
                    fixture.verify_face_pick(source,index,view,viewport,int(pixel.X),int(pixel.Y),
                        shaded=operation.get('display','Shaded')!='Wireframe')
                    host['_record_progress']('verified extraction face frustum %d:%d viewport=%s point=%d,%d' %
                        (source,index,str(viewport.Id),int(pixel.X),int(pixel.Y)))
            before=fixture.snapshot();components=fixture.components()
            marker='Viboceros ExtractSrf '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
            script='_ExtractSrf _Copy=_%s _OutputLayer=_%s %s' % (
                'Yes' if operation['copy'] else 'No','Current' if operation['output_current'] else 'Input',
                '_Pause' if steps else '_Enter')
            if steps:
                from shrink_face_input import drive
                action=lambda:drive(operation,points,host,fixture.components,trace,'ExtractSrf',script,completion)
            else:action=lambda:Rhino.RhinoApp.RunScript(script,True)
            ok,after,events=observe_command(Rhino.Commands.Command,'ExtractSrf',
                action,fixture.snapshot,lambda:[],True)
            history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)
            if len(history)!=2:raise ValueError('ExtractSrf history marker missing')
            for event in events:event.pop('objects',None)
            result=dict(constructed=constructed,before=before,after=after,succeeded=ok,events=events,
                history=history[1].strip(),component_selection=dict(before=components,after=fixture.components()))
            if steps:
                result['selection_steps']=trace
                result['input_completion']=completion
            changed=[{k:v for k,v in o.items() if k!='selected'} for o in before]!=[{k:v for k,v in o.items() if k!='selected'} for o in after]
            if operation['undo_redo'] and changed:
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
