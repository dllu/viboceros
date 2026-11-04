"""A private idle-event worker for real mesh/SubD preselection gestures."""
import json
import os
import time
import Rhino
import System
from edge_loop_probe import Input
from edge_loop_gesture_cases import validate

root=os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(root,'request.json')) as stream:request=json.load(stream)
if type(request.get('iterations')) is not int or request['iterations']!=1 or request.get('protocol_version')!=1:raise ValueError('owned gesture protocol required')
for operation in request['operations']:validate(operation)
state=dict(index=0,phase='setup',busy=False,value=None,results=[],trace=[],finished=False)

def progress(message):
    with open(os.path.join(root,'worker-progress.log'),'a') as stream:stream.write(message+'\n')

class Mouse(Rhino.UI.MouseCallback):
    def OnEndMouseUp(self,event):
        if state['value'] is not None:state['trace'].append(state['value'].selected())
mouse=Mouse()

def cleanup():
    mouse.Enabled=False
    value=state['value'];state['value']=None
    if value is not None:value.__exit__(None,None,None)

def finish(error=None):
    if state['finished']:return
    state['finished']=True
    errors=[error] if error else []
    try:Rhino.RhinoApp.Idle-=idle
    except Exception as failure:errors.append(str(failure))
    try:cleanup()
    except Exception as failure:errors.append(str(failure))
    response=dict(protocol_version=1,engine='rhino',engine_version=str(Rhino.RhinoApp.Version),iterations=1,results=state['results'])
    if errors:response.update(error='; '.join(errors),results=[])
    with open(os.path.join(root,'response.json.tmp'),'w') as stream:json.dump(response,stream)
    os.rename(os.path.join(root,'response.json.tmp'),os.path.join(root,'response.json'))
    progress('worker: response published');Rhino.RhinoDoc.ActiveDoc.Modified=False
    Rhino.RhinoApp.RunScript('_Exit _No',False)

def idle(sender,event):
    if state['busy'] or state['finished']:return
    state['busy']=True
    try:
        if state['index']==len(request['operations']):finish();return
        op=request['operations'][state['index']]
        if Rhino.Commands.Command.InCommand():return
        if state['phase']=='setup':
            if list(Rhino.RhinoDoc.ActiveDoc.Objects):raise ValueError('empty owned gesture document required')
            Rhino.RhinoApp.RunScript('_SelectionFilterNone',False)
            value=Input(op,globals());state['value']=value;value.__enter__()
            if not Rhino.RhinoApp.RunScript('_SelectionFilterEdges',False):raise ValueError('edge-only selection filter failed')
            if op['op']=='edge_loop_subobject_gesture':Rhino.ApplicationSettings.SelectionFilterSettings.SubObjectSelect=False
            obj=value.doc.Objects.FindId(value.key)
            for pair in op['initial']:
                index=next(i for i,e in enumerate(value.edges) if set(e)==set(pair))
                kind=Rhino.Geometry.ComponentIndexType.MeshTopologyEdge if op['kind']=='mesh' else Rhino.Geometry.ComponentIndexType.SubdEdge
                if not obj.SelectSubObject(Rhino.Geometry.ComponentIndex(kind,value.native[index]),True,True,False):raise ValueError('cannot seed owned loop anchor')
            value.before=value.selected();state['trace']=[];mouse.Enabled=True;value.doc.Views.Redraw()
            view=value.doc.Views.ActiveView;pixel=view.ActiveViewport.WorldToClient(value.point)
            screen=view.ClientToScreen(System.Drawing.Point(int(pixel.X),int(pixel.Y)))
            state.update(phase='wait',started=time.time())
            progress('PICK @edge-loop-double:%s %d %d'%(op['id'],screen.X,screen.Y))
        elif state['phase']=='wait':
            if time.time()-state['started']>15:raise ValueError('owned loop gesture timed out')
            path=os.path.join(root,'click-ack.json')
            if not os.path.isfile(path):return
            with open(path) as stream:ack=json.load(stream)
            if ack!='@edge-loop-double:'+op['id']:return
            state.update(phase='record',ready=time.time()+.6)
        elif time.time()>=state['ready']:
            value=state['value'];mouse.Enabled=False;after=value.selected()
            if len(state['trace'])!=2:raise ValueError('double click did not deliver two viewport releases')
            unchanged=int(value.doc.Objects.FindId(value.key).Geometry.DataCRC(0))==value.crc
            order=[]
            if after['ordinals']:
                if not Rhino.RhinoApp.RunScript('_-ToNURBS _DeleteInputObjects=_No _Enter',False):raise ValueError('gesture conversion failed')
                points=value.recipe['points']
                for obj in sorted((o for o in value.doc.Objects if o.Id!=value.key),key=lambda o:o.RuntimeSerialNumber):
                    curve=obj.Geometry
                    if not isinstance(curve,Rhino.Geometry.NurbsCurve) or curve.Degree!=1:raise ValueError('unexpected flat gesture curve')
                    pair=[min(range(len(points)),key=lambda i:p.DistanceTo(Rhino.Geometry.Point3d(*points[i]))) for p in (curve.PointAtStart,curve.PointAtEnd)]
                    order.append(next(i for i,e in enumerate(value.edges) if set(e)==set(pair)))
                if set(order)!=set(after['ordinals']) or len(order)!=len(after['ordinals']):raise ValueError('gesture conversion coverage differs')
            record=dict(input=value.recipe,archive=value.archive,edges=value.edges,native=value.native,
                        seed=value.seed,before=value.before,trace=state['trace'],after=after,source_unchanged=unchanged,filter='edges',to_nurbs_order=order,subobject_select=Rhino.ApplicationSettings.SelectionFilterSettings.SubObjectSelect)
            state['results'].append(dict(id=op['id'],value=record,elapsed_ns=0))
            progress('GESTURE %s %s'%(op['id'],json.dumps(after)))
            cleanup();state['index']+=1;state['phase']='setup'
    except Exception as error:
        progress('PICK_ERROR '+str(error));finish(op['id']+' '+type(error).__name__+': '+str(error))
    finally:state['busy']=False

Rhino.RhinoApp.Idle+=idle
