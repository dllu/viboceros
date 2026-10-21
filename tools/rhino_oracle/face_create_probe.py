"""Fixed direct mesh-face command sequences in an owned idle document."""
from copy import deepcopy

QUAD=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]]
TRIANGLE=QUAD[:3]
PENTAGON=[[0.,0.,0.],[3.,0.,0.],[4.,2.,0.],[2.,4.,0.],[-1.,2.,0.]]
CASES={
 'quad':dict(points=QUAD),
 'triangle':dict(points=TRIANGLE,polygon='Triangle'),
 'triangle_enter':dict(points=TRIANGLE),
 'reverse':dict(points=list(reversed(QUAD))),
 'concave':dict(points=[[0.,0.,0.],[4.,0.,0.],[1.,1.,0.],[0.,3.,0.]]),
 'twisted':dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,2.],[0.,3.,0.]]),
 'in_plane':dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,2.],[0.,3.,0.]],in_plane=True),
 'ngon':dict(points=PENTAGON,polygon='Ngon'),
 'ngon_triangle':dict(points=TRIANGLE,polygon='Ngon'),
 'ngon_quad':dict(points=QUAD,polygon='Ngon'),
 'ngon_concave':dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[2.,1.,0.],[0.,4.,0.]],polygon='Ngon'),
 'ngon_spatial':dict(points=[[0.,0.,0.],[3.,0.,1.],[4.,2.,0.],[2.,4.,1.],[-1.,2.,0.]],polygon='Ngon'),
 'multiple':dict(points=QUAD+[[4.,0.,0.],[7.,0.,0.],[7.,3.,0.],[4.,3.,0.]],multiple=True),
 'undo_pick':dict(points=QUAD[:3]+['Undo',[4.,3.,0.],QUAD[3]]),
 'cancel_partial':dict(points=QUAD[:2],cancel=True),
}
for _name,_height,_angle in [('append_flat',0.,180.),('append_fold',3.,180.),('append_unweld',3.,0.),('append_crease',3.,30.)]:
 CASES[_name]=dict(points=[[4.,0.,0.],[7.,0.,_height],[7.,3.,_height],[4.,3.,0.]],append=True,weld_angle=_angle)
CANCEL_CASES={'cancel_partial':CASES.pop('cancel_partial')}
SCENES=CASES.copy();SCENES.update(CANCEL_CASES)
VARIANTS={}
for _name,_height,_angle in [('flat_zero',0.,0.),('fold_44',3.,44.),('fold_45',3.,45.),('fold_46',3.,46.)]:
 VARIANTS[_name]=dict(points=[[4.,0.,0.],[7.,0.,_height],[7.,3.,_height],[4.,3.,0.]],append=True,weld_angle=_angle)
for _name,_gap in [('near_seam',2e-7),('far_seam',2e-5)]:
 VARIANTS[_name]=dict(points=[[4.+_gap,0.,0.],[7.,0.,0.],[7.,3.,0.],[4.+_gap,3.,0.]],append=True)
VARIANTS['old_unwelded']=dict(points=[[4.,0.,0.],[7.,0.,0.],[7.,3.,0.],[4.,3.,0.]],append=True,
 source_vertices=[[0.,0.,0.],[2.,0.,0.],[2.,3.,0.],[0.,3.,0.],[2.,0.,0.],[4.,0.,0.],[4.,3.,0.],[2.,3.,0.]],source_faces=[[0,1,2,3],[4,5,6,7]])
SCENES.update(VARIANTS)
CONTROLS={
 'edge_single':dict(append=True,edge_pick=[4.,1.5,0.],points=[[7.,0.,0.],[7.,3.,0.]]),
 'cancel_partial':dict(points=QUAD[:2],cancel=True),
 'append_cancel_partial':dict(append=True,points=QUAD[:2],cancel=True),
 'multiple_cancel_partial':dict(multiple=True,points=QUAD+[[5.,0.,0.],[7.,0.,0.]],cancel=True),
 'append_after_face_cancel_partial':dict(append=True,multiple=True,points=[[4.,0.,0.],[7.,0.,0.],[7.,3.,0.],[4.,3.,0.],[8.,0.,0.],[9.,0.,0.]],cancel=True),
}
SCENES.update(CONTROLS)
if __package__:from .face_draft_cases import ALL_DRAFTS as DRAFTS
else:from face_draft_cases import ALL_DRAFTS as DRAFTS
SCENES.update(DRAFTS)


def request():
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
             operations=[dict(op='face_create_command',id=name,case=name)for name in CASES])


def variant_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in VARIANTS];return q


def control_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in CONTROLS];return q


def validate(op):
 try:
  valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='face_create_command'and op['case']in SCENES  and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed 3DFace case required')


def run(op,host,iterations,tolerance):
 validate(op)
 from join_probe import observe_command
 R=host['Rhino'];G=R.Geometry;S=host['System'];doc=R.RhinoDoc.ActiveDoc
 if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
 case=deepcopy(SCENES[op['case']]);sources=[];layers=[];old_layer=doc.Layers.CurrentLayerIndex
 def snapshot():
  rows=[]
  for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
   row=dict(role='source'if obj.Id in sources else'output',selected=bool(obj.IsSelected(False)),layer=doc.Layers[obj.Attributes.LayerIndex].Name,name=obj.Attributes.Name,crc=int(obj.Geometry.DataCRC(0)))
   if isinstance(obj.Geometry,G.Mesh):
    row['mesh']=host['_polygon_mesh_value'](obj.Geometry)
    row['ngons']=[dict(vertices=[int(i)for i in n.BoundaryVertexIndexList()],faces=[int(i)for i in n.FaceIndexList()])for n in obj.Geometry.Ngons]
    row['normal_count']=int(obj.Geometry.Normals.Count)
    if case.get('native_validity'):
     valid,log=obj.Geometry.IsValidWithLog();row['native_validity']=dict(valid=bool(valid),diagnostics=str(log or''))
     row['vertex_normals']=[[float(n.X),float(n.Y),float(n.Z)]for n in obj.Geometry.Normals]
     row['face_normals']=[[float(n.X),float(n.Y),float(n.Z)]for n in obj.Geometry.FaceNormals]
   rows.append(row)
  return rows
 try:
  record=doc.BeginUndoRecord('Owned 3DFace setup')
  try:
   for name in ['3DFace source','3DFace output']:
    layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
   if case.get('append'):
    mesh=G.Mesh()
    for p in case.get('source_vertices',QUAD):mesh.Vertices.Add(*p)
    for f in case.get('source_faces',[[0,1,2,3]]):mesh.Faces.AddFace(*f)
    mesh.Normals.ComputeNormals()
    a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input'
    sources.append(doc.Objects.AddMesh(mesh,a));mesh.Dispose()
    doc.Objects.Select(sources[0])
   doc.Layers.SetCurrentLayerIndex(layers[1],True)
  finally:doc.EndUndoRecord(record)
  doc.ClearUndoRecords(True);before=snapshot()
  marker='Owned 3DFace '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
  command='_-3DFace _Output=Mesh _PolygonType='+case.get('polygon','Quad')
  command+=' _InPlane='+('Yes'if case.get('in_plane')else'No')
  command+=' _WeldAngle='+str(case.get('weld_angle',180.))
  command+=' _Mode='+('MultipleFaces'if case.get('multiple')else'SingleFace')
  command+=' _FromEdge='+('Yes'if op['case']in DRAFTS and case.get('from_edge',True)else'No'if op['case']in DRAFTS else'Yes _Pause'if case.get('edge_pick')else'No')
  if op['case']in DRAFTS and case.get('from_edge',True)and case.get('polygon','Quad')=='Quad':command+=' _Chain='+('Yes'if case.get('chain')else'No')
  for point in case['points']:
   if isinstance(point,dict):command+=' _Pause'
   elif isinstance(point,str):command+=' _'+point
   else:command+=' w'+','.join(format(float(v),'.17g')for v in point)
  if case.get('cancel'):command+=' _Pause'
  else:command+=' _Enter _Enter _Cancel _Cancel _Cancel'if op['case']in DRAFTS else' _Enter _Cancel _Cancel _Cancel'
  trace={}
  def execute():
   if op['case']in DRAFTS:
    from face_draft_input import drive
    result,observed=drive(op,case,command,host);trace.update(observed);return result
   if op['case']in CONTROLS:
    from face_create_input import drive
    result,observed=drive(op,case,command,host);trace.update(observed);return result
   return R.RhinoApp.RunScript(command,True)
  success,after,events=observe_command(R.Commands.Command,'3DFace',execute,snapshot,lambda:[],True)
  history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
  R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
  return dict(input=case,success=success,events=events,history=history,document_tolerance=doc.ModelAbsoluteTolerance,
              before=before,after=after,after_script=after_script,undo=undo,redo=redo,**(dict(input_trace=trace)if trace else{})),0
 finally:
  doc.Layers.SetCurrentLayerIndex(old_layer,True)
  for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
  for layer in reversed(layers):doc.Layers.Delete(layer,True)
