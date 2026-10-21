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
SUBD_CASES={}
for _name in ['quad','triangle','triangle_enter','reverse','concave','twisted','in_plane','ngon','ngon_spatial','multiple']:
 SUBD_CASES['subd_'+_name]=dict(deepcopy(CASES[_name]),output='SubD',interpolated=False)
SUBD_SURFACE_CASES={name.replace("subd_","subd_surface_",1):dict(deepcopy(case),surface_mesh=True)for name,case in SUBD_CASES.items()}
SCENES.update(SUBD_CASES)
SCENES.update(SUBD_SURFACE_CASES)
SUBD_APPEND_CASES={}
for _name in ['append_flat','append_fold','multiple','ngon','triangle','undo_pick']:
 _case=deepcopy(CASES[_name]);_case.update(append=True,source_subd=True,output='SubD',interpolated=False,surface_mesh=True)
 if _name=='multiple':_case['points']=[[4.,0.,0.],[7.,0.,0.],[7.,3.,0.],[4.,3.,0.],[7.,0.,0.],[10.,0.,1.],[10.,3.,1.],[7.,3.,0.]]
 if _name=='ngon':_case['points']=[[4.,0.,0.],[7.,-1.,0.],[9.,1.,1.],[7.,4.,0.],[4.,3.,0.]]
 if _name=='triangle':_case['points']=[[4.,0.,0.],[7.,1.,2.],[4.,3.,0.]]
 if _name=='undo_pick':_case['points']=[[4.,0.,0.],[7.,0.,0.],[7.,3.,0.],'Undo',[7.,3.,2.],[4.,3.,0.]]
 SUBD_APPEND_CASES['subd_append_'+(_name[7:]if _name.startswith('append_')else _name)]=_case
for _name,_gap in [('near',2e-7),('far',2e-5)]:
 SUBD_APPEND_CASES['subd_append_'+_name]=dict(deepcopy(SUBD_APPEND_CASES['subd_append_flat']),points=[[4.+_gap,0.,0.],[7.,0.,0.],[7.,3.,0.],[4.+_gap,3.,0.]])
SUBD_APPEND_CASES['subd_append_corner']=dict(deepcopy(SUBD_APPEND_CASES['subd_append_fold']),source_corners=[1])
SUBD_APPEND_CASES['subd_append_crease']=dict(deepcopy(SUBD_APPEND_CASES['subd_append_fold']),source_vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,3.,0.],[2.,3.,0.],[4.,3.,0.]],source_faces=[[0,1,4,3],[1,2,5,4]],source_creases=[[1,4]])
SUBD_APPEND_CASES['subd_append_reverse']=dict(deepcopy(SUBD_APPEND_CASES['subd_append_fold']),points=list(reversed(SUBD_APPEND_CASES['subd_append_fold']['points'])))
SUBD_APPEND_CASES['subd_append_disconnected']=dict(deepcopy(SUBD_APPEND_CASES['subd_append_flat']),points=[[8.,0.,0.],[10.,0.,0.],[10.,3.,0.],[8.,3.,0.]])
SCENES.update(SUBD_APPEND_CASES)
SUBD_APPEND_EXTRAS={
 'subd_extra_corner_unaffected':dict(deepcopy(SUBD_APPEND_CASES['subd_append_fold']),source_corners=[0]),
 'subd_extra_corner_both':dict(deepcopy(SUBD_APPEND_CASES['subd_append_fold']),source_corners=[1,2]),
 'subd_extra_vertex_only':dict(deepcopy(SUBD_APPEND_CASES['subd_append_flat']),points=[[4.,3.,0.],[7.,3.,0.],[7.,6.,0.],[4.,6.,0.]]),
 'subd_extra_late_bridge':dict(deepcopy(SUBD_APPEND_CASES['subd_append_multiple']),points=[[8.,3.,0.],[12.,3.,0.],[12.,0.,0.],[8.,0.,0.],[4.,0.,0.],[8.,0.,0.],[8.,3.,0.],[4.,3.,0.]]),
 'subd_extra_standalone_disjoint':dict(deepcopy(SUBD_APPEND_CASES['subd_append_multiple']),append=False,points=QUAD+[[8.,0.,0.],[12.,0.,0.],[12.,3.,0.],[8.,3.,0.]]),
 'subd_extra_standalone_bridge':dict(deepcopy(SUBD_APPEND_CASES['subd_append_multiple']),append=False,points=QUAD+[[8.,3.,0.],[12.,3.,0.],[12.,0.,0.],[8.,0.,0.],[4.,0.,0.],[8.,0.,0.],[8.,3.,0.],[4.,3.,0.]]),
 'subd_extra_ngon_source':dict(deepcopy(SUBD_APPEND_CASES['subd_append_flat']),source_vertices=PENTAGON,source_faces=[[0,1,2],[0,2,3],[0,3,4]],source_ngons=[dict(vertices=[0,1,2,3,4],faces=[0,1,2])],points=[[3.,0.,0.],[7.,0.,0.],[7.,2.,0.],[4.,2.,0.]]),
 'subd_extra_interior_edge':dict(deepcopy(SUBD_APPEND_CASES['subd_append_crease']),source_creases=[],polygon='Triangle',surface_mesh=False,points=[[2.,0.,0.],[3.,1.5,2.],[2.,3.,0.]]),
 'subd_extra_output_mesh':dict(deepcopy(SUBD_APPEND_CASES['subd_append_flat']),output='Mesh'),
 'subd_extra_mesh_source':dict(deepcopy(SUBD_APPEND_CASES['subd_append_flat']),source_subd=False),
}
SCENES.update(SUBD_APPEND_EXTRAS)
SUBD_DRAFT_CASES={}
for _name in ['chain_single','edge_multiple','chain_strip','chain_reverse','chain_fold','chain_undo','chain_new_boundary','edge_triangles','edge_ngons','chain_cancel']:
 _case=deepcopy(DRAFTS[_name]);_case.update(source_subd=True,output='SubD',interpolated=False,surface_mesh=True)
 SUBD_DRAFT_CASES['subd_draft_'+_name]=_case
SCENES.update(SUBD_DRAFT_CASES)
PHYSICAL_DRAFTS=set(DRAFTS)|set(SUBD_DRAFT_CASES)
SUBD_NORMAL_CASES={
 'subd_normal_quad':dict(deepcopy(SUBD_SURFACE_CASES['subd_surface_quad']),native_normals=True),
 'subd_normal_twisted':dict(deepcopy(SUBD_SURFACE_CASES['subd_surface_twisted']),native_normals=True),
 'subd_normal_ngon':dict(deepcopy(SUBD_SURFACE_CASES['subd_surface_ngon_spatial']),native_normals=True),
 'subd_normal_crease':dict(deepcopy(SUBD_APPEND_CASES['subd_append_crease']),native_normals=True,source_vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,.5],[0.,3.,0.],[2.,3.,0.],[4.,3.,.5]],points=[[4.,0.,.5],[7.,0.,.75],[7.,3.,.75],[4.,3.,.5]]),
 'subd_normal_corner':dict(deepcopy(SUBD_APPEND_EXTRAS['subd_extra_corner_unaffected']),native_normals=True),
 'subd_normal_nonmanifold':dict(deepcopy(SUBD_APPEND_EXTRAS['subd_extra_interior_edge']),native_normals=True,surface_mesh=True),
}
SUBD_NORMAL_CASES['subd_normal_smooth']=dict(deepcopy(SUBD_NORMAL_CASES['subd_normal_crease']),source_creases=[])
SCENES.update(SUBD_NORMAL_CASES)





def request():
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
             operations=[dict(op='face_create_command',id=name,case=name)for name in CASES])


def variant_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in VARIANTS];return q


def subd_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in SUBD_CASES];return q


def subd_surface_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in SUBD_SURFACE_CASES];return q


def subd_append_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in SUBD_APPEND_CASES];return q


def subd_append_extra_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in SUBD_APPEND_EXTRAS];return q


def subd_draft_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in SUBD_DRAFT_CASES];return q


def subd_normal_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in SUBD_NORMAL_CASES];return q


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
   elif isinstance(obj.Geometry,G.SubD):
    subd=obj.Geometry
    vertices=[];vertex=subd.Vertices.First
    while vertex is not None:
     vertices.append(vertex);vertex=vertex.Next
    index={int(v.Id):i for i,v in enumerate(vertices)}
    point=lambda p:[float(p.X),float(p.Y),float(p.Z)]
    row['subd']=dict(points=[point(v.ControlNetPoint)for v in vertices],vertex_tags=[int(v.Tag)for v in vertices],surface_points=[point(v.SurfacePoint())for v in vertices],
                     edges=[[index[int(e.VertexFrom.Id)],index[int(e.VertexTo.Id)]]for e in subd.Edges],edge_tags=[int(e.Tag)for e in subd.Edges],
                     faces=[[index[int(f.VertexAt(i).Id)]for i in range(f.EdgeCount)]for f in subd.Faces],valid=bool(subd.IsValid))
    if case.get('surface_mesh'):
     mesh=G.Mesh.CreateFromSubD(subd,2)
     if mesh is None:raise ValueError('SubD limit meshing failed')
     try:
      row['subd']['surface_mesh']=host['_polygon_mesh_value'](mesh)
      if case.get('native_normals'):
       if mesh.Normals.Count!=mesh.Vertices.Count:raise ValueError('missing native limit mesh normals')
       row['subd']['surface_mesh_normals']=[point(n)for n in mesh.Normals]
     finally:mesh.Dispose()
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
    for ngon in case.get('source_ngons',[]):mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](ngon['vertices']),S.Array[S.UInt32](ngon['faces'])))
    mesh.Normals.ComputeNormals()
    a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input'
    if case.get('source_subd'):
     subd=G.SubD.CreateFromMesh(mesh)
     if subd is None:raise ValueError('source SubD creation failed')
     vertex=subd.Vertices.First;vertices=[]
     while vertex is not None:
      vertices.append(vertex);vertex=vertex.Next
     for index in case.get('source_corners',[]):vertices[index].Tag=G.SubDVertexTag.Corner
     index={int(v.Id):i for i,v in enumerate(vertices)}
     for edge in subd.Edges:
      if sorted([index[int(edge.VertexFrom.Id)],index[int(edge.VertexTo.Id)]])in case.get('source_creases',[]):edge.Tag=G.SubDEdgeTag.Crease
     subd.UpdateAllTagsAndSectorCoefficients()
     if not subd.IsValid:raise ValueError('invalid source SubD')
     sources.append(doc.Objects.AddSubD(subd,a));subd.Dispose()
    else:sources.append(doc.Objects.AddMesh(mesh,a))
    mesh.Dispose()
    doc.Objects.Select(sources[0])
   doc.Layers.SetCurrentLayerIndex(layers[1],True)
  finally:doc.EndUndoRecord(record)
  doc.ClearUndoRecords(True);before=snapshot()
  marker='Owned 3DFace '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
  command='_-3DFace _Output='+case.get('output','Mesh')
  if case.get('output')=='SubD':command+=' _Interpolated='+('Yes'if case.get('interpolated')else'No')
  command+=' _PolygonType='+case.get('polygon','Quad')
  command+=' _InPlane='+('Yes'if case.get('in_plane')else'No')
  if case.get('output','Mesh')=='Mesh':command+=' _WeldAngle='+str(case.get('weld_angle',180.))
  command+=' _Mode='+('MultipleFaces'if case.get('multiple')else'SingleFace')
  command+=' _FromEdge='+('Yes'if op['case']in PHYSICAL_DRAFTS and case.get('from_edge',True)else'No'if op['case']in PHYSICAL_DRAFTS else'Yes _Pause'if case.get('edge_pick')else'No')
  if op['case']in PHYSICAL_DRAFTS and case.get('from_edge',True)and case.get('polygon','Quad')=='Quad':command+=' _Chain='+('Yes'if case.get('chain')else'No')
  for point in case['points']:
   if isinstance(point,dict):command+=' _Pause'
   elif isinstance(point,str):command+=' _'+point
   else:command+=' w'+','.join(format(float(v),'.17g')for v in point)
  if case.get('cancel'):command+=' _Pause'
  else:command+=' _Enter _Enter _Cancel _Cancel _Cancel'if op['case']in PHYSICAL_DRAFTS else' _Enter _Cancel _Cancel _Cancel'
  trace={}
  def execute():
   if op['case']in PHYSICAL_DRAFTS:
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
