"""Fixed public SubDBox command recipes in a private idle document."""
from copy import deepcopy

SCENES = {
 'subd_box_default': dict(actions=[[0.,0.,0.],[4.,3.,0.],5.]),
 'subd_box_one': dict(counts=[1,1,1],actions=[[0.,0.,0.],[4.,3.,0.],5.]),
 'subd_box_two': dict(counts=[2,2,2],actions=[[0.,0.,0.],[4.,3.,0.],5.]),
 'subd_box_counts': dict(counts=[2,3,4],actions=[[1.,2.,3.],[5.,8.,3.],7.]),
 'subd_box_reversed': dict(counts=[2,3,1],actions=[[5.,8.,3.],[1.,2.,3.],-4.]),
 'subd_box_height_point': dict(counts=[1,2,3],actions=[[0.,0.,2.],[4.,3.,7.],[9.,-2.,6.]]),
 'subd_box_height_default': dict(counts=[2,1,3],actions=[[0.,0.,0.],[4.,3.,0.],'Enter']),
 'subd_box_center': dict(counts=[2,3,1],actions=['Center',[1.,2.,3.],[3.,5.,3.],4.]),
 'subd_box_three_point': dict(counts=[2,3,1],actions=['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.],4.]),
 'subd_box_midpoint': dict(counts=[2,3,1],actions=['EdgeMidpoint',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.],4.]),
 'subd_box_vertical': dict(counts=[2,3,1],actions=['Vertical',[1.,2.,3.],[4.,6.,3.],[0.,5.,8.],4.]),
 'subd_box_diagonal': dict(counts=[2,3,1],actions=['Diagonal',[1.,2.,3.],[5.,8.,10.]]),
 'subd_box_diagonal_flat': dict(counts=[2,3,1],actions=['Diagonal',[1.,2.,3.],[5.,8.,3.],4.]),
 'subd_box_cube': dict(counts=[2,3,1],actions=['Cube',[1.,2.,3.],[5.,8.,10.]]),
 'subd_box_cube_flat': dict(counts=[2,3,1],actions=['Cube',[1.,2.,3.],[5.,8.,3.]]),
 'subd_box_uniform_count': dict(actions=[3,[0.,0.,0.],[4.,3.,0.],5.]),
 'subd_box_dimensions': dict(counts=[2,3,1],actions=[[0.,0.,0.],4.,3.,5.]),
 'subd_box_around_curve': dict(counts=[2,3,1],curve=[[0.,0.,0.],[0.,0.,10.]],actions=['AroundCurve',{'curve':True},[0.,0.,2.],[2.,3.,2.],4.]),
 'subd_box_tilted': dict(counts=[2,3,1],construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]],actions=[[10.,20.,30.],[12.4,23.2,36.],-5.]),
 'subd_box_tilted_diagonal': dict(counts=[2,3,1],construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]],actions=['Diagonal',[10.,20.,30.],[8.4,26.2,36.]]),
 'subd_box_cancel': dict(counts=[2,3,1],actions=[[0.,0.,0.],[4.,3.,0.],'Cancel'],cancel=True),
}

for _name in ['midpoint','diagonal','diagonal_flat','cube','cube_flat','tilted_diagonal']:
 SCENES['subd_box_'+_name]['unsupported']=True
SCENES['subd_box_zero_origin']=dict(counts=[1,2,3],actions=[0,[4.,3.,0.],5.])
SCENES['subd_box_around_x']=dict(counts=[2,3,1],curve=[[0.,0.,0.],[10.,0.,0.]],actions=['AroundCurve',{'curve':True},[2.,0.,0.],[2.,2.,3.],4.])

def request():
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_box_command',id=name,case=name)for name in SCENES])

def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_box_command'and op['case']in SCENES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDBox recipe required')

def subd_value(subd,host):
 vertices=[];vertex=subd.Vertices.First
 while vertex is not None:vertices.append(vertex);vertex=vertex.Next
 index={int(v.Id):i for i,v in enumerate(vertices)};point=lambda p:[float(p.X),float(p.Y),float(p.Z)]
 mesh=host['Rhino'].Geometry.Mesh.CreateFromSubD(subd,2)
 if mesh is None:raise ValueError('native limit mesh failed')
 try:return dict(points=[point(v.ControlNetPoint)for v in vertices],vertex_tags=[int(v.Tag)for v in vertices],surface_points=[point(v.SurfacePoint())for v in vertices],edges=[[index[int(e.VertexFrom.Id)],index[int(e.VertexTo.Id)]]for e in subd.Edges],edge_tags=[int(e.Tag)for e in subd.Edges],faces=[[index[int(f.VertexAt(i).Id)]for i in range(f.EdgeCount)]for f in subd.Faces],valid=bool(subd.IsValid),surface_mesh=host['_polygon_mesh_value'](mesh))
 finally:mesh.Dispose()

def run(op,host,iterations,tolerance):
 validate(op)
 return run_scene(op,host,iterations,tolerance,'SubDBox',SCENES[op['case']])

def run_scene(op,host,iterations,tolerance,command,recipe):
 from join_probe import observe_command
 R=host['Rhino'];G=R.Geometry;S=host['System'];doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
 if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty private idle document required')
 case=deepcopy(recipe);saved_plane=vp.GetConstructionPlane();old_layer=doc.Layers.CurrentLayerIndex;layer=-1;guide=None
 def snapshot():
  rows=[]
  for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
   row=dict(role='source'if obj.Id==guide else'output',selected=bool(obj.IsSelected(False)),layer=doc.Layers[obj.Attributes.LayerIndex].Name,name=obj.Attributes.Name,crc=int(obj.Geometry.DataCRC(0)))
   if isinstance(obj.Geometry,G.SubD):row['subd']=subd_value(obj.Geometry,host)
   elif isinstance(obj.Geometry,G.Point):row['point']=host['_xyz'](obj.Geometry.Location)
   else:row['curve_points']=[host['_xyz'](obj.Geometry.PointAtStart),host['_xyz'](obj.Geometry.PointAtEnd)]
   rows.append(row)
  return rows
 try:
  origin,x,y=case.get('construction_plane',[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]])
  vp.SetConstructionPlane(G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y)))
  serial=doc.BeginUndoRecord('Owned '+command+' setup')
  try:
   layer=doc.Layers.Add(command+' output',S.Drawing.Color.Blue);doc.Layers.SetCurrentLayerIndex(layer,True)
   if 'curve'in case:
    g=G.LineCurve(G.Point3d(*case['curve'][0]),G.Point3d(*case['curve'][1]))
    try:guide=doc.Objects.AddCurve(g)
    finally:g.Dispose()
  finally:doc.EndUndoRecord(serial)
  doc.ClearUndoRecords(True);before=snapshot();tokens=['_-'+command]
  for name,count in zip(['XCount','YCount','ZCount'],([]if case.get('counts_at_end')else case.get('counts',[]))):tokens.append('_'+name+'='+str(count))
  for action in case['actions']:
   if isinstance(action,dict):tokens.extend(["'_-SelID",str(guide)])
   elif isinstance(action,str):tokens.append('_'+action)
   elif isinstance(action,list):tokens.append('w'+','.join(format(float(v),'.17g')for v in action))
   else:tokens.append(format(float(action),'.17g'))
  tokens.extend(['_Cancel']*3);macro=' '.join(tokens);marker='Owned '+command+' '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
  success,after,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
  history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
  R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
  return dict(input=case,success=success,events=events,history=history,macro=macro,before=before,after=after,after_script=after_script,undo=undo,redo=redo),0
 finally:
  vp.SetConstructionPlane(saved_plane);doc.Layers.SetCurrentLayerIndex(old_layer,True)
  for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
  if layer>=0:doc.Layers.Delete(layer,True)
