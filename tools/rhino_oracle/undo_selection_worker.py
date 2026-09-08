"""Bounded command-history probes in a newly owned Rhino document at idle."""
import Rhino, System, os, json
root = os.path.dirname(os.path.abspath(__file__))
d = Rhino.RhinoDoc.ActiveDoc
with open(os.path.join(root,'request.json')) as f: request=json.load(f)
state={'busy':False,'done':False,'index':0,'results':[],'owned':set(),'baseline':None}
def log(s):
    try:
        with open(os.path.join(root,'worker-progress.log'),'a') as f: f.write(s+'\n')
    except Exception: pass  # Optional diagnostics must not abort finalization.
def objects(): return list(d.Objects.GetObjectList(Rhino.DocObjects.ObjectType.AnyObject))
def own(key):
    if key == System.Guid.Empty: raise ValueError('object insertion failed')
    state['owned'].add(key)
    return key
def run_command(command):
    if not Rhino.RhinoApp.RunScript(command,False):
        raise ValueError('command failed: '+command)
def capture(source,other):
    all_objects=[o for o in objects() if o.Id not in state['baseline']]
    state['owned'].update(o.Id for o in all_objects)
    def info(key):
        o=d.Objects.FindId(key)
        if o is None: return None
        g=o.Geometry
        return dict(selected=bool(o.IsSelected(False)),kind=str(o.ObjectType),faces=int(g.Faces.Count) if isinstance(g,Rhino.Geometry.Mesh) else None)
    return dict(source=info(source),other=info(other),count=len(all_objects),selected_outputs=sum(bool(o.IsSelected(False)) for o in all_objects if o.Id not in [source,other]))
def cleanup():
    owned=state['owned'];state['owned']=set()
    errors=[]
    try: d.Objects.UnselectAll()
    except Exception as error: errors.append('deselect: '+str(error))
    for key in owned:
        try:
            if d.Objects.FindId(key) is not None and not d.Objects.Delete(key,True): raise ValueError('cleanup failed')
        except Exception as error: errors.append(str(error))
    if errors: raise ValueError('; '.join(errors))
def finish(error=None):
    if state['done']: return
    state['done']=True
    try: Rhino.RhinoApp.Idle -= idle
    except Exception as e: error=str(e) if error is None else error+'; '+str(e)
    # Without a successful baseline scan, no enumerated object is known to be
    # ours. Explicitly registered insertions are always safe to clean up.
    if state['baseline'] is not None:
        try: state['owned'].update(o.Id for o in objects() if o.Id not in state['baseline'])
        except Exception as e: error=str(e) if error is None else error+'; '+str(e)
    try: cleanup()
    except Exception as e: error=str(e) if error is None else error+'; '+str(e)
    response=dict(protocol_version=1,engine='rhino',engine_version=str(Rhino.RhinoApp.Version),iterations=1,results=state['results'])
    if error: response.update(error=error,results=[])
    with open(os.path.join(root,'response.json.tmp'),'w') as f: json.dump(response,f)
    os.rename(os.path.join(root,'response.json.tmp'),os.path.join(root,'response.json'))
    try:
        d.Modified=False
        Rhino.RhinoApp.RunScript('_Exit _No',False)
    except Exception: pass  # The owned-process client provides termination fallback.
def idle(sender,event):
    if state['busy'] or state['done']: return
    state['busy']=True
    try:
        if state['index']==len(request['operations']): finish();return
        op=request['operations'][state['index']];kind=op['kind'];log('case '+op['id'])
        state['baseline']=None
        state['baseline']=set(o.Id for o in objects())
        if kind in ['DeleteFaces','ExtractMeshFaces']:
            mesh=Rhino.Geometry.Mesh()
            try:
                for x,y,z in [(0,0,0),(2,0,0),(0,2,0),(2,2,0),(1,1,1)]: mesh.Vertices.Add(x,y,z)
                for a,b,c in [(0,1,4),(1,3,4),(3,2,4),(2,0,4)]: mesh.Faces.AddFace(a,b,c)
                source=own(d.Objects.AddMesh(mesh))
            finally: mesh.Dispose()
        elif kind=='Explode':
            source=own(d.Objects.AddPolyline([Rhino.Geometry.Point3d(x,y,0) for x,y in [(0,0),(4,0),(4,3),(0,3),(0,0)]]))
        else: source=own(d.Objects.AddLine(Rhino.Geometry.Point3d(0,0,0),Rhino.Geometry.Point3d(2,0,0)))
        other=own(d.Objects.AddPoint(Rhino.Geometry.Point3d(10,10,0)))
        d.Objects.UnselectAll()
        if kind in ['DeleteFaces','ExtractMeshFaces']:
            component=Rhino.Geometry.ComponentIndex(Rhino.Geometry.ComponentIndexType.MeshFace,0)
            if d.Objects.FindId(source).SelectSubObject(component,True,True,False)==0: raise ValueError('subobject selection failed')
        elif not d.Objects.Select(source): raise ValueError('selection failed')
        records=[]
        command={'Move':'_Move w0,0,0 w0,1,0','Delete':'_Delete','Explode':'_Explode','DeleteFaces':'_DeleteFaces _Enter','ExtractMeshFaces':'_ExtractMeshFaces _Enter'}[kind]
        run_command(command);records.append(capture(source,other));log('command completed')
        if op['clear']: d.Objects.UnselectAll()
        if not d.Objects.Select(other): raise ValueError('unrelated object selection failed')
        records.append(capture(source,other))
        for cmd in ['_Undo','_Redo','_Undo']:
            run_command(cmd);records.append(capture(source,other));log(cmd+' completed')
        state['results'].append(dict(id=op['id'],elapsed_ns=0,value=dict(succeeded=True,states=records)))
        cleanup();state['index']+=1
    except Exception as e: finish(str(e))
    finally: state['busy']=False
log('worker started')
Rhino.RhinoApp.Idle += idle
