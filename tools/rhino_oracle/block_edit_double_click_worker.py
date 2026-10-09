# -*- coding: utf-8 -*-
import Rhino,System,os,json,time
root=os.path.dirname(os.path.abspath(__file__))
request=json.load(open(os.path.join(root,'request.json')))
doc=Rhino.RhinoDoc.ActiveDoc
state={'stage':'setup','busy':False,'started':time.time()}
def progress(value):
    with open(os.path.join(root,'worker-progress.log'),'a') as stream:stream.write(value+'\n')
def objects():
    settings=Rhino.DocObjects.ObjectEnumeratorSettings();settings.HiddenObjects=True;settings.LockedObjects=True
    return [o for o in doc.Objects.GetObjectList(settings) if not o.IsDeleted and not o.IsInstanceDefinitionGeometry]
def finish(value,error=None):
    Rhino.RhinoApp.Idle-=idle
    response=dict(protocol_version=1,engine='rhino',engine_version=str(Rhino.RhinoApp.Version),iterations=1,results=[dict(id=request['operations'][0]['id'],elapsed_ns=0,value=value)])
    if error:response.update(error=error,results=[])
    with open(os.path.join(root,'response.json.tmp'),'w') as stream:json.dump(response,stream)
    os.rename(os.path.join(root,'response.json.tmp'),os.path.join(root,'response.json'));progress('worker: response published')
    doc.Modified=False;Rhino.RhinoApp.Exit(False)
def idle(sender,args):
    if state['busy']:return
    state['busy']=True
    try:
        if state['stage']=='setup':
            geometry=Rhino.Geometry.LineCurve(Rhino.Geometry.Point3d(0,0,0),Rhino.Geometry.Point3d(10,0,0))
            attributes=Rhino.DocObjects.ObjectAttributes();attributes.Name='DoubleClickMember'
            index=doc.InstanceDefinitions.Add('VibocerosOracleDoubleClick','',Rhino.Geometry.Point3d.Origin,[geometry],[attributes])
            if index<0:raise ValueError('definition admission failed')
            identifier=doc.Objects.AddInstanceObject(index,Rhino.Geometry.Transform.Identity)
            state.update(identifier=identifier,before=set(o.Id for o in objects()),index=index)
            Rhino.RhinoApp.RunScript('_SetView _World _Top',False);Rhino.RhinoApp.RunScript('_Zoom _Extents',False)
            view=doc.Views.ActiveView;doc.Views.Redraw()
            point=view.ActiveViewport.WorldToClient(Rhino.Geometry.Point3d(5,0,0));screen=view.ClientToScreen(System.Drawing.Point(int(point.X),int(point.Y)))
            state['stage']='wait';progress('PICK '+request['operations'][0]['id']+' '+str(screen.X)+' '+str(screen.Y))
        elif state['stage']=='wait':
            ack=os.path.join(root,'click-ack.json')
            if not os.path.isfile(ack):return
            exposed=[o for o in objects() if o.Id not in state['before']]
            if not exposed:
                if time.time()-os.path.getmtime(ack)<3:return
                raise ValueError('double click did not expose block members: '+str(Rhino.RhinoApp.CommandHistoryWindowText)[-1000:])
            value=dict(opened=True,exposed=len(exposed),root_visible=bool(doc.Objects.FindId(state['identifier']).Attributes.Visible),member_names=[str(o.Attributes.Name) for o in exposed])
            if not Rhino.RhinoApp.RunScript('_-BlockEdit _DiscardAndCancel',False):raise ValueError('discard failed')
            value['restored_model']=set(o.Id for o in objects())==state['before']
            value['definition_members']=len(doc.InstanceDefinitions[state['index']].GetObjects())
            finish(value)
    except Exception as error:finish({},str(error))
    finally:state['busy']=False
progress('worker: started');Rhino.RhinoApp.Idle+=idle
