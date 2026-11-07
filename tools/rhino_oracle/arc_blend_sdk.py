"""Fixed point/tangent recipes for the public two-arc blend constructor."""
CASES={}
def add(name,end,t0,t1,ratios):
 for i,r in enumerate(ratios):CASES[name+'_'+str(i)]=dict(start=[0.,0.,0.],start_direction=t0,end=end,end_direction=t1,ratio=float(r))
add('corner',[5.,4.,0.],[1.,0.,0.],[0.,1.,0.],[0.,.25,.5,1.,2.,4.,-1.,-.5])
add('s_shape',[10.,3.,0.],[1.,0.,0.],[1.,0.,0.],[0.,.25,1.,4.])
add('straight',[10.,0.,0.],[1.,0.,0.],[1.,0.,0.],[0.,1.,2.,-1.])
add('opposed',[10.,0.,0.],[1.,0.,0.],[-1.,0.,0.],[0.,1.,2.])
add('spatial',[7.,4.,3.],[1.,2.,-1.],[-1.,1.,2.],[0.,.25,1.,4.])
add('back_start',[5.,4.,0.],[-1.,0.,0.],[0.,1.,0.],[0.,1.,2.,-1.])
add('back_both',[5.,4.,0.],[-1.,0.,0.],[0.,-1.,0.],[0.,1.,2.])
CASES['coincident']=dict(start=[0.,0.,0.],start_direction=[1.,0.,0.],end=[0.,0.,0.],end_direction=[0.,1.,0.],ratio=1.)
CASES['zero_direction']=dict(start=[0.,0.,0.],start_direction=[0.,0.,0.],end=[5.,4.,0.],end_direction=[0.,1.,0.],ratio=1.)
PILOT=list(CASES)
from math import sqrt
unit=lambda v:[x/sqrt(sum(y*y for y in v))for x in v]
HOLDOUTS={}
for name,point,t0,t1 in [('unit_spatial',[7.,4.,3.],[1.,2.,-1.],[-1.,1.,2.]),('skew_space',[4.,-2.,6.],[2.,-1.,3.],[-2.,4.,1.])]:
 for i,r in enumerate([.1,.25,1.,4.]):HOLDOUTS[name+'_'+str(i)]=dict(start=[0.,0.,0.],start_direction=unit(t0),end=point,end_direction=unit(t1),ratio=r)
CASES.update(HOLDOUTS)
RADIUS_SEARCH={}
for name,point,t0,t1 in [('corner',[5.,4.,0.],[1.,0.,0.],[0.,1.,0.]),('skew',[10.,4.,0.],[1.,0.,0.],[4.,-2.,0.]),('spatial',[7.,4.,3.],[1.,2.,-1.],[-1.,1.,2.])]:
 for i in range(1,17):
  a=(1./101.)+(i/17.)*(99./101.)
  RADIUS_SEARCH['search_'+name+'_'+str(i)]=dict(start=[0.,0.,0.],start_direction=unit(t0),end=point,end_direction=unit(t1),ratio=a/(1.-a))
CASES.update(RADIUS_SEARCH)
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='arc_blend_sdk'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed arc blend query')
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='arc_blend_sdk',id=k,case=k)for k in(PILOT if names is None else names)])
def run(op,host,iterations,tolerance):
 validate(op)
 R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
 if iterations!=1 or list(doc.Objects)or R.Commands.Command.InCommand():raise ValueError('owned empty idle SDK scene required')
 recipe=CASES[op['case']];curve=G.Curve.CreateArcBlend(G.Point3d(*recipe['start']),G.Vector3d(*recipe['start_direction']),G.Point3d(*recipe['end']),G.Vector3d(*recipe['end_direction']),recipe['ratio'])
 if curve is None:return dict(case=recipe,success=False),0
 def observe(c):
  value=dict(native_type=c.GetType().Name,native_valid=bool(c.IsValid),curve=host['_nurbs_curve_definition'](c),samples=[dict(parameter=c.Domain.ParameterAt(i/32.),point=host['_xyz'](c.PointAt(c.Domain.ParameterAt(i/32.))),tangent=host['_xyz'](c.TangentAt(c.Domain.ParameterAt(i/32.))))for i in range(33)])
  if isinstance(c,G.ArcCurve):
   a=c.Arc;value['arc']=dict(center=host['_xyz'](a.Center),radius=a.Radius,x_axis=host['_xyz'](a.Plane.XAxis),y_axis=host['_xyz'](a.Plane.YAxis),angles=[a.AngleDomain.T0,a.AngleDomain.T1],domain=[c.Domain.T0,c.Domain.T1])
  return value
 try:
  value=observe(curve);value.update(case=recipe,success=True)
  if isinstance(curve,G.PolyCurve):value['segments']=[observe(curve.SegmentCurve(i))for i in range(curve.SegmentCount)]
  else:value['segments']=[observe(curve)]
  return value,0
 finally:curve.Dispose()
