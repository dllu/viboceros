"""Fixed public PolylineOnMesh getters on independently defined meshes."""
import re
from copy import deepcopy
MESHES={
 'plane':([[-4.,-4.,0.],[4.,-4.,0.],[4.,4.,0.],[-4.,4.,0.]],[[0,1,2,3]]),
 'fold':([[-4.,-4.,0.],[0.,-4.,0.],[4.,-4.,3.],[-4.,4.,0.],[0.,4.,0.],[4.,4.,3.]],[[0,1,4,3],[1,2,5,4]]),
 'ridge':([[-4.,-4.,0.],[0.,-4.,2.],[4.,-4.,0.],[-4.,4.,0.],[0.,4.,2.],[4.,4.,0.]],[[0,1,4,3],[1,2,5,4]]),
 'twisted':([[0.,0.,0.],[4.,0.,0.],[4.,4.,2.],[0.,4.,0.]],[[0,1,2,3]]),
 'disconnected':([[-4.,-4.,0.],[-1.,-4.,0.],[-1.,4.,0.],[-4.,4.,0.],[1.,-4.,0.],[4.,-4.,0.],[4.,4.,0.],[1.,4.,0.]],[[0,1,2,3],[4,5,6,7]]),
}
A=[[-3.,-2.,0.],[2.,-1.,0.],[1.,3.,0.],[-2.,2.,0.]]
CASES={
 'plane_two':('plane',[A[0],A[1],'Enter']),
 'plane_many':('plane',A+['Enter']),
 'off_mesh':('plane',[[-6.,-2.,4.],[1.,1.,3.],[6.,3.,-2.],'Enter']),
 'close':('plane',A+['Close']),
 'undo':('plane',A[:3]+['Undo']+A[3:]+['Enter']),
 'cancel':('plane',A[:2]+['Cancel']),
 'fold':('fold',[[-3.,-2.,0.],[3.,2.,2.25],'Enter']),
 'fold_reverse':('fold',[[3.,2.,2.25],[-3.,-2.,0.],'Enter']),
 'ridge':('ridge',[[-3.,-1.,.5],[3.,2.,.5],'Enter']),
 'twisted':('twisted',[[.5,.5,0.],[3.5,3.5,1.75],'Enter']),
 'disconnected':('disconnected',[[-3.,0.,0.],[3.,1.,0.],'Enter']),
 'return_to_start':('plane',A[:3]+A[:1]),
}
MESHES['hole']=([[-4.,-4.,0.],[4.,-4.,0.],[4.,4.,0.],[-4.,4.,0.],[-1.,-1.,0.],[1.,-1.,0.],[1.,1.,0.],[-1.,1.,0.]],[[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]])
MESHES['disconnected_reversed']=(deepcopy(MESHES['disconnected'][0]),[list(reversed(f))for f in MESHES['disconnected'][1]])
MESHES['three_panels']=([[x,y,0.]for lo,hi in[(-6.,-3.),(-1.,1.),(3.,6.)]for x,y in[(lo,-4.),(hi,-4.),(hi,4.),(lo,4.)]],[[4*i,4*i+1,4*i+2,4*i+3]for i in range(3)])
MESHES['unwelded_fold']=([[-4.,-4.,0.],[0.,-4.,0.],[0.,4.,0.],[-4.,4.,0.],[0.,-4.,0.],[4.,-4.,3.],[4.,4.,3.],[0.,4.,0.]],[[0,1,2,3],[4,5,6,7]])
MESHES['coplanar_grid']=([[-4.,-4.,0.],[0.,-4.,0.],[4.,-4.,0.],[-4.,4.,0.],[0.,4.,0.],[4.,4.,0.]],[[0,1,4,3],[1,2,5,4]])
BOUNDARY_CASES={
 'hole_center':('hole',[[-3.,0.,0.],[3.,0.,0.],'Enter']),
 'hole_reverse':('hole',[[3.,0.,0.],[-3.,0.,0.],'Enter']),
 'hole_upper':('hole',[[-3.,.8,0.],[3.,.8,0.],'Enter']),
 'hole_lower':('hole',[[-3.,-.8,0.],[3.,-.8,0.],'Enter']),
 'hole_slope':('hole',[[-3.,-.5,0.],[3.,.5,0.],'Enter']),
 'panels_reverse':('disconnected',[[3.,1.,0.],[-3.,0.,0.],'Enter']),
 'panels_lower':('disconnected',[[-3.,-2.,0.],[3.,-1.,0.],'Enter']),
 'panels_normals_reversed':('disconnected_reversed',[[-3.,0.,0.],[3.,1.,0.],'Enter']),
 'three_panels':('three_panels',[[-5.,0.,0.],[5.,1.,0.],'Enter']),
 'unwelded_fold':('unwelded_fold',[[-3.,-2.,0.],[3.,2.,2.25],'Enter']),
 'coplanar_grid':('coplanar_grid',[[-3.,-2.,0.],[3.,2.,0.],'Enter']),
}
TIE_CASES={
 'slope_reverse':('hole',[[3.,.5,0.],[-3.,-.5,0.],'Enter']),
 'slope_dyadic':('hole',[[-3.,-.75,0.],[3.,.75,0.],'Enter']),
 'slope_negative':('hole',[[-3.,.5,0.],[3.,-.5,0.],'Enter']),
 'slope_negative_reverse':('hole',[[3.,-.5,0.],[-3.,.5,0.],'Enter']),
 'hole_vertical':('hole',[[0.,-3.,0.],[0.,3.,0.],'Enter']),
 'hole_vertical_reverse':('hole',[[0.,3.,0.],[0.,-3.,0.],'Enter']),
}
SCENES=dict(CASES,**BOUNDARY_CASES)
SCENES.update(TIE_CASES)
def tie_request():
 q=request();q['operations']=[dict(op='polyline_on_mesh',id=k,case=k,actions=deepcopy(v[1]))for k,v in TIE_CASES.items()];return q
def boundary_request():
 q=request();q['operations']=[dict(op='polyline_on_mesh',id=k,case=k,actions=deepcopy(v[1]))for k,v in BOUNDARY_CASES.items()];return q
def request():
 return deepcopy(dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='polyline_on_mesh',id=k,case=k,actions=v[1])for k,v in CASES.items()]))
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case','actions'}or op['op']!='polyline_on_mesh'or not isinstance(op['case'],str)or op['case']not in SCENES:raise ValueError('fixed mesh polyline case required')
 if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or op['actions']!=SCENES[op['case']][1]:raise ValueError('fixed mesh polyline actions required')
def validate_request(q):
 if type(q.get('iterations'))is not int or q['iterations']!=1 or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=16:raise ValueError('bounded one-iteration mesh polyline request required')
 for op in q['operations']:validate(op)
 if len({op['id']for op in q['operations']})!=len(q['operations']):raise ValueError('unique mesh polyline ids required')
def run(op,host,iterations,tolerance):
 validate(op)
 from join_probe import observe_command
 from conic_getter_input import drive
 R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
 if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
 vertices,faces=MESHES[SCENES[op['case']][0]];mesh=G.Mesh()
 for p in vertices:mesh.Vertices.Add(*p)
 for f in faces:mesh.Faces.AddFace(*f)
 mesh.Normals.ComputeNormals();mesh.Compact()
 old=doc.Layers.CurrentLayerIndex;layers=[];identifier=None
 def snapshot():
  rows=[]
  for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
   row=dict(role='target'if o.Id==identifier else'output',selected=bool(o.IsSelected(False)),layer=doc.Layers[o.Attributes.LayerIndex].Name,name=o.Attributes.Name,crc=int(o.Geometry.DataCRC(0)))
   if isinstance(o.Geometry,G.Curve):
    row['curve']=host['_nurbs_curve_definition'](o.Geometry);ok,polyline=o.Geometry.TryGetPolyline();row['polyline']=[host['_xyz'](p)for p in polyline]if ok else None
   rows.append(row)
  return rows
 try:
  record=doc.BeginUndoRecord('Owned mesh polyline source')
  try:
   for name in ['Mesh polyline source','Mesh polyline output']:
    layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
   a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Target';identifier=doc.Objects.AddMesh(mesh,a)
   doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.Objects.Select(identifier)
  finally:doc.EndUndoRecord(record)
  doc.ClearUndoRecords(True);before=snapshot();traces=[]
  marker='Owned PolylineOnMesh '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
  def execute():
   success,trace=drive(op,host,[identifier],'PolylineOnMesh');traces.append(trace);return success
  success,after,events=observe_command(R.Commands.Command,'PolylineOnMesh',execute,snapshot,lambda:[],True)
  after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
  R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
  return dict(mesh=dict(vertices=[host['_xyz'](mesh.Vertices.Point3dAt(i))for i in range(mesh.Vertices.Count)],faces=faces),success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,input_trace=traces[0],events=events,history=history),0
 finally:
  doc.Layers.SetCurrentLayerIndex(old,True)
  for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
  for index in reversed(layers):doc.Layers.Delete(index,True)
  mesh.Dispose()
