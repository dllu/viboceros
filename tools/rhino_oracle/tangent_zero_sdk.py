"""Compare public zero-finder experiments with public tangent-point queries."""
from math import sqrt
CASES={
 'zero_arc':dict(type='arc',points=[[5.,0.,0.],[3.5355339059327378,3.5355339059327378,0.],[0.,5.,0.]]),
 'zero_nurbs':dict(type='nurbs',degree=2,knots=[0.,0.,0.,1.,1.,1.],control_points=[dict(point=p,weight=w)for p,w in zip([[5.,0.,0.],[5.,5.,0.],[0.,5.,0.]],[1.,1./sqrt(2),1.])]),
}
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='tangent_zero_sdk'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed zero-search experiment')
def request():return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='tangent_zero_sdk',id=k,case=k)for k in CASES])
def run(op,host,iterations,tolerance):
 validate(op)
 R=host['Rhino'];G=R.Geometry
 if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects)or R.Commands.Command.InCommand():raise ValueError('owned empty idle SDK scene required')
 from tangent_zero_trial import root
 curve=host['_object_source'](CASES[op['case']],tolerance)
 def dot(a,b):return sum(a[i]*b[i]for i in range(3))
 def cross(a,b):return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
 def evaluate(t):
  derivatives=curve.DerivativeAt(t,2)
  if derivatives is None or len(derivatives)!=3:raise ValueError('public curve derivative evaluation failed')
  p=derivatives[0]
  d,dd=map(host['_xyz'],derivatives[1:]);tangent=host['_xyz'](curve.TangentAt(t));speed=derivatives[1].Length
  tangent_derivative=[(dd[i]-tangent[i]*dot(tangent,dd))/speed for i in range(3)]
  r=[v-q for v,q in zip([-8.,6.,0.],host['_xyz'](p))]
  normal=cross(tangent,[0.,0.,1.]);dn=cross(tangent_derivative,[0.,0.,1.])
  return dict(unit_cross=(cross(r,tangent)[2],cross(r,tangent_derivative)[2]),dot_normal=(dot(r,normal),dot(r,dn)),derivative_cross=(cross(r,d)[2],cross(r,dd)[2]))
 try:
  a,b=curve.Domain.T0,curve.Domain.T1;rows=[]
  for i in range(1001):
   seed=curve.Domain.ParameterAt(i/1000.);ok,parameter=curve.GetLocalTangentPoint(G.Point3d(-8.,6.,0.),seed);trials=[]
   values=evaluate(seed)
   for name in ['unit_cross','dot_normal','derivative_cross']:
    for bracket in ['full','local']:
     lo,hi=(a,b)if bracket=='full'else(seed,b)if values[name][0]<0. else(a,seed)
     result,why=root(lambda t:evaluate(t)[name],lo,hi,0.,(b-a)*1.4901161193847656e-8)
     trials.append(dict(function=name,bracket=bracket,parameter=result,reason=why))
   rows.append(dict(seed=seed,native_success=bool(ok),native_parameter=float(parameter),trials=trials))
  return dict(source=CASES[op['case']],domain=[a,b],queries=rows),0
 finally:curve.Dispose()
