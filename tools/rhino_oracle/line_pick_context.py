"""Fixed tangent picks measured by the public viewport picking context."""
if __package__:from .line_tangent_rays import CIRCLE,TILTED,CUBIC,ARC,case
else:from line_tangent_rays import CIRCLE,TILTED,CUBIC,ARC,case
CASES={
 'pick_circle_perspective':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],view='Perspective',before='FromFirstPoint'),
 'pick_tilted_perspective':case(TILTED,[6.99,-2.8,4.15],[10.,1.8,7.6],view='Perspective',before='FromFirstPoint'),
 'pick_cubic_perspective':case(CUBIC,[3.52,1.84,2.8],[6.22,1.39,3.85],view='Perspective',before='FromFirstPoint'),
 'pick_circle_top':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],before='FromFirstPoint'),
}
PILOT=list(CASES)
HOLDOUTS={
 'pick_circle_far':case(CIRCLE,[-4.99,.2,3.],[8.,6.,3.],view='Perspective',before='FromFirstPoint'),
 'pick_circle_quarter':case(CIRCLE,[.2,4.99,3.],[8.,6.,3.],view='Perspective',before='FromFirstPoint'),
 'pick_tilted_top':case(TILTED,[6.99,-2.8,4.15],[10.,1.8,7.6],before='FromFirstPoint'),
 'pick_tilted_front':case(TILTED,[6.99,-2.8,4.15],[10.,1.8,7.6],view='Front',before='FromFirstPoint'),
 'pick_cubic_front':case(CUBIC,[3.52,1.84,2.8],[6.22,1.39,3.85],view='Front',before='FromFirstPoint'),
 'pick_arc_perspective':case(ARC,[3.5355339059327378,3.5355339059327378,3.],[-8.,6.,3.],view='Perspective',before='FromFirstPoint'),
 'pick_fixed_bothsides':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],view='Perspective',before='FromFirstPoint',both=True),
 'pick_fixed_planar':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],view='Perspective',before='FromFirstPoint',planar=True),
}
for name,source,pick,aim,length in [('pick_circle_length',CIRCLE,[4.99,.2,3.],[8.,6.,3.],5.),('pick_cubic_negative',CUBIC,[3.52,1.84,2.8],[6.22,1.39,3.85],-5.)]:
 recipe=case(source,pick,aim,view='Perspective',before='FromFirstPoint');recipe['actions'].insert(-1,length);HOLDOUTS[name]=recipe
CASES.update(HOLDOUTS)
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='line_pick_context'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed public picking recipe')
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_pick_context',id=k,case=k)for k in(PILOT if names is None else names)])
def measure(source,vp,view,pixel,host):
 R,S=host['Rhino'],host['System'];point=S.Drawing.Point(int(pixel.X),int(pixel.Y));transform=vp.GetPickTransform(point)
 context=R.Input.Custom.PickContext();curve=source.ToNurbsCurve()
 try:
  context.View=view;context.PickStyle=R.Input.Custom.PickStyle.PointPick;context.PickMode=R.Input.Custom.PickMode.Wireframe
  context.PickLine=vp.ClientToWorld(point);context.SetPickTransform(transform);context.UpdateClippingPlanes()
  ok,t,depth,distance=context.PickFrustumTest(curve)
  matrix=[[float(transform[i,j])for j in range(4)]for i in range(4)]
  return dict(success=bool(ok),parameter=float(t),point=host['_xyz'](curve.PointAt(t))if ok else None,depth=float(depth),distance=float(distance),transform=matrix,
   domain=[curve.Domain.T0,curve.Domain.T1],mode=str(context.PickMode),style=str(context.PickStyle),camera_location=host['_xyz'](vp.CameraLocation),camera_direction=host['_xyz'](vp.CameraDirection),camera_up=host['_xyz'](vp.CameraUp))
 finally:context.Dispose();curve.Dispose()
