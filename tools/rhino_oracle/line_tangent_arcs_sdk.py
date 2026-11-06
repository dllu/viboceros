"""Bounded dense public contact queries, independent of interactive Line output."""
from math import sqrt
ARC=dict(type='arc',points=[[5.,0.,0.],[3.5355339059327378,3.5355339059327378,0.],[0.,5.,0.]])
NURBS=dict(type='nurbs',degree=2,knots=[0.,0.,0.,1.,1.,1.],control_points=[dict(point=p,weight=w)for p,w in zip([[5.,0.,0.],[5.,5.,0.],[0.,5.,0.]],[1.,1./sqrt(2),1.])])
def record(source,target=(-8.,6.,0.)):return dict(source=source,target=list(target))
CASES={
 'dense_arc':record(ARC),
 'dense_nurbs':record(NURBS),
 'dense_reparameterized':record(dict(NURBS,knots=[1000.,1000.,1000.,2000.,2000.,2000.])),
 'dense_reversed':record(dict(ARC,points=list(reversed(ARC['points'])))),
 'dense_rotated45':record(dict(ARC,points=[[(p[0]-p[1])/sqrt(2),(p[0]+p[1])/sqrt(2),p[2]]for p in ARC['points']]),target=[-14./sqrt(2),-2./sqrt(2),0.]),
 'dense_yz':record(dict(ARC,points=[[0.,p[0],p[1]]for p in ARC['points']]),target=[0.,-8.,6.]),
 'dense_scaled':record(dict(ARC,points=[[.5*x for x in p]for p in ARC['points']]),target=[-4.,3.,0.]),
 'dense_other_target':record(ARC,target=[-6.,8.,0.]),
}
PILOT=list(CASES)
BOUNDS={
 'bounds_left01':dict(record(ARC),subdomain=[.1,1.]),
 'bounds_left02':dict(record(ARC),subdomain=[.2,1.]),
 'bounds_right095':dict(record(ARC),subdomain=[0.,.95]),
 'bounds_right099':dict(record(ARC),subdomain=[0.,.99]),
 'bounds_nurbs':dict(record(NURBS),subdomain=[.1,1.]),
 'bounds_refined':dict(record(NURBS),refine=True),
 'bounds_far_target':record(ARC,target=[-10.,6.,0.]),
 'bounds_near_target':record(ARC,target=[-8.,5.5,0.]),
 'bounds_high_target':record(ARC,target=[-8.,7.,0.]),
}
CASES.update(BOUNDS)
FRACTIONS=[i/1000. for i in range(1001)]
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='line_tangent_arcs_sdk'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed tangent SDK query')
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_tangent_arcs_sdk',id=k,case=k)for k in (PILOT if names is None else names)])
def run(op,host,iterations,tolerance):
 validate(op)
 R=host['Rhino']
 if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():raise ValueError('owned empty idle SDK scene required')
 from line_tangent_arcs_probe import contacts
 case=CASES[op['case']]
 return dict(case=case,contacts=contacts(host,case['source'],case['target'],[0.,0.,0.],tolerance,FRACTIONS,case.get('subdomain'),case.get('refine',False))),0
