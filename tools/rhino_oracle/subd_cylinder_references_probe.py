"""Owned SubDCylinder tangent and point-fit reference input, sources and history."""
from copy import deepcopy
PREFIX=['Solid=Yes','AroundFaces=8','CapFaceStyle=Quad','VerticalFaces=2']
LINES=[dict(type='line',start=[0.,0.,0.],end=[12.,0.,0.]),dict(type='line',start=[0.,0.,0.],end=[0.,12.,0.]),dict(type='line',start=[0.,10.,0.],end=[10.,0.,0.])]
CARDINAL=[[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.],[0.,-2.,0.]]
def scene(actions,**kw):return dict(actions=PREFIX+actions,**kw)
def pick(i,p):return dict(curve=i,pick=p)
SCENES={
 'cylinder_tangent_points':scene(['Tangent','Point',[3.,0.,0.],'Point',[0.,3.,0.],'Point',[-3.,0.,0.],5.]),
 'cylinder_tangent_spatial':scene(['Tangent','Point',[1.,2.,3.],'Point',[4.,6.,3.],'Point',[0.,5.,5.],4.]),
 'cylinder_tangent_two':scene(['Tangent',pick(0,[3.,0.,0.]),pick(1,[0.,4.,0.]),'Enter',5.],curves=LINES[:2],live=True),
 'cylinder_tangent_radius':scene(['Tangent',pick(0,[3.,0.,0.]),3.,pick(1,[0.,4.,0.]),5.],curves=LINES[:2],live=True),
 'cylinder_tangent_late_radius':scene(['Tangent',pick(0,[3.,0.,0.]),pick(1,[0.,4.,0.]),'Radius',3.,5.],curves=LINES[:2],live=True),
 'cylinder_tangent_from_first':scene(['Tangent',pick(0,[3.,0.,0.]),'FromFirstPoint',pick(1,[0.,4.,0.]),5.],curves=LINES[:2],live=True),
 'cylinder_tangent_three':scene(['Tangent',pick(0,[3.,0.,0.]),pick(1,[0.,4.,0.]),pick(2,[5.,5.,0.]),5.],curves=LINES,live=True),
 'cylinder_tangent_cancel_contact':scene(['Tangent',pick(0,[3.,0.,0.]),'Cancel'],curves=LINES[:2],live=True,cancel=True),
 'cylinder_tangent_cancel_height':scene(['Tangent','Point',[3.,0.,0.],'Point',[0.,3.,0.],'Point',[-3.,0.,0.],'Cancel'],cancel=True),
 'cylinder_fit_preselected':scene(['FitPoints',5.],points=CARDINAL[:3],selected=[0,1,2]),
 'cylinder_fit_postselected':scene(['FitPoints','SelAll','Enter',5.],points=CARDINAL),
 'cylinder_fit_partial':scene(['FitPoints','SelAll','Enter',-5.],points=CARDINAL,selected=[0,2]),
 'cylinder_fit_spatial':scene(['FitPoints',4.],points=[[1.,2.,3.],[4.,6.,3.],[0.,5.,5.],[0.1,1.5,3.8]],selected=[0,1,2,3]),
 'cylinder_fit_collinear':scene(['FitPoints','Cancel'],points=[[0.,0.,0.],[1.,0.,0.],[2.,0.,0.]],selected=[0,1,2],no_output=True),
 'cylinder_fit_insufficient':scene(['FitPoints','SelAll','Enter','Cancel'],points=CARDINAL[:2],cancel=True),
 'cylinder_fit_cancel_selection':scene(['FitPoints','Cancel'],points=CARDINAL,cancel=True),
 'cylinder_fit_cancel_height':scene(['FitPoints','SelAll','Enter','Cancel'],points=CARDINAL,cancel=True),
 'cylinder_fit_grips':scene(['FitPoints',5.],controls=CARDINAL,weights=[1.,2.,.5,1.],selected_grips=[0,1,2,3]),
 'cylinder_fit_mixed':scene(['FitPoints',5.],controls=CARDINAL,selected_grips=[0],points=CARDINAL[1:3],selected=[0,1]),
 'cylinder_fit_grips_postselected':scene(['FitPoints','SelAll','Enter',5.],controls=CARDINAL),
}
EXTRA_CASES={
 'cylinder_fit_collinear_finish':scene(['FitPoints',5.],points=[[0.,0.,0.],[1.,0.,0.],[2.,0.,0.]],selected=[0,1,2],no_output=True),
 'cylinder_fit_coincident_finish':scene(['FitPoints',5.],points=[[1.,2.,3.]]*4,selected=[0,1,2,3],no_output=True),
}
ALL_CASES=dict(SCENES);ALL_CASES.update(EXTRA_CASES)
def request(extras=False):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_cylinder_references',id=k,case=k)for k in (EXTRA_CASES if extras else SCENES)])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_cylinder_references'and op['case']in ALL_CASES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDCylinder reference recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from join_probe import observe_command
 from subd_box_probe import subd_value
 R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;view=doc.Views.ActiveView;vp=view.ActiveViewport
 if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
 case=deepcopy(ALL_CASES[op['case']]);ids=[];point_ids=[];grip_id=None;layer=-1
 old_plane=vp.GetConstructionPlane();old_view=R.DocObjects.ViewportInfo(vp);old_layer=doc.Layers.CurrentLayerIndex
 def snapshot():
  rows=[]
  for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
   g=obj.Geometry;row=dict(role='source'if obj.Id in ids else'output',source=ids.index(obj.Id)if obj.Id in ids else None,selected=bool(obj.IsSelected(False)),name=obj.Attributes.Name,layer=doc.Layers[obj.Attributes.LayerIndex].Name,crc=int(g.DataCRC(0)))
   if isinstance(g,G.SubD):row['subd']=subd_value(g,host)
   elif isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
   else:
    row['curve']=host['_nurbs_curve_definition'](g)
    row['grips_on']=bool(obj.GripsOn);row['grips']=[dict(point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False)))for p in(obj.GetGrips()or[])]
   rows.append(row)
  return rows
 try:
  vp.SetConstructionPlane(G.Plane.WorldXY)
  serial=doc.BeginUndoRecord('Owned SubDCylinder references')
  try:
   layer=doc.Layers.Add('SubDCylinder output',S.Drawing.Color.Blue);doc.Layers.SetCurrentLayerIndex(layer,True)
   for source in case.get('curves',[]):
    g=G.LineCurve(G.Point3d(*source['start']),G.Point3d(*source['end']))
    try:ids.append(doc.Objects.AddCurve(g))
    finally:g.Dispose()
   if 'controls'in case:
    g=G.NurbsCurve(3,True,3,len(case['controls']))
    for i,p in enumerate(case['controls']):g.Points.SetPoint(i,G.Point3d(*p),case.get('weights',[1.]*len(case['controls']))[i])
    g.Knots.CreateUniformKnots(1.)
    try:grip_id=doc.Objects.AddCurve(g);ids.append(grip_id)
    finally:g.Dispose()
   for p in case.get('points',[]):point_ids.append(doc.Objects.AddPoint(G.Point3d(*p)));ids.append(point_ids[-1])
   for i,id in enumerate(ids):
    obj=doc.Objects.FindId(id);a=obj.Attributes.Duplicate();a.Name='reference '+str(i);doc.Objects.ModifyAttributes(id,a,True)
  finally:doc.EndUndoRecord(serial)
  if grip_id is not None:
   obj=doc.Objects.FindId(grip_id);obj.GripsOn=True
   for i in case.get('selected_grips',[]):obj.GetGrips()[i].Select(True)
  for i in case.get('selected',[]):doc.Objects.Select(point_ids[i])
  doc.ClearUndoRecords(True);before=snapshot();marker='Owned cylinder references '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);trace=[]
  if case.get('live'):
   from conic_getter_input import drive
   vp.SetToPlanView(G.Point3d.Origin,G.Vector3d.XAxis,G.Vector3d.YAxis,True);vp.ZoomExtents();view.Redraw()
   def execute():
    success,t=drive(dict(op='subd_cylinder_references',id=op['id'],actions=case['actions']),host,ids,command='SubDCylinder');trace.append(t);return success
   macro=None
  else:
   tokens=['_-SubDCylinder']
   for a in case['actions']:
    if isinstance(a,list):tokens.append('w'+','.join(format(float(x),'.17g')for x in a))
    elif isinstance(a,str):tokens.append('_'+a)
    else:tokens.append(str(a))
   macro=' '.join(tokens);execute=lambda:R.RhinoApp.RunScript(macro,True)
  success,after,events=observe_command(R.Commands.Command,'SubDCylinder',execute,snapshot,lambda:[],True)
  history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
  R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
  return dict(input=case,success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history,macro=macro,input_trace=trace),0
 finally:
  vp.SetViewProjection(old_view,False);old_view.Dispose();vp.SetConstructionPlane(old_plane);doc.Layers.SetCurrentLayerIndex(old_layer,True)
  for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
  if layer>=0:doc.Layers.Delete(layer,True)
