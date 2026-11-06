"""Independent open-arc stations, representations and rigid/scale variants."""
from math import cos,sin,pi,sqrt
ARC=dict(type='arc',points=[[5,0,0],[5/sqrt(2),5/sqrt(2),0],[0,5,0]])
def case(pick,target=(-8,6,0),source=None,*before):
 return dict(curves=[ARC if source is None else source],actions=['Tangent',dict(curve=0,pick=pick)]+list(before)+[list(target)])
CASES={'arc_station_'+str(i):case([5*cos(a),5*sin(a),0])for i,a in enumerate([.2,.4,.6,.7,.78,.8,1.,1.2,1.4])}
CASES.update({
 'arc_middle_control':case(ARC['points'][1]),
 'arc_reversed':case(ARC['points'][1],source=dict(type='arc',points=list(reversed(ARC['points'])))),
 'arc_as_nurbs':case(ARC['points'][1],source=dict(type='nurbs',degree=2,knots=[0,0,0,1,1,1],control_points=[dict(point=p,weight=w)for p,w in zip([[5,0,0],[5,5,0],[0,5,0]],[1,1/sqrt(2),1])])),
 'arc_translated':case([20+5/sqrt(2),-30+5/sqrt(2),4],target=[12,-24,4],source=dict(type='arc',points=[[p[0]+20,p[1]-30,4]for p in ARC['points']])),
 'arc_rotated':case([-5/sqrt(2),5/sqrt(2),0],target=[-6,-8,0],source=dict(type='arc',points=[[-p[1],p[0],0]for p in ARC['points']])),
 'arc_scaled':case([2.5/sqrt(2),2.5/sqrt(2),0],target=[-4,3,0],source=dict(type='arc',points=[[x/2 for x in p]for p in ARC['points']])),
 'arc_hover_contact':case(ARC['points'][1],(-8,6,0),None,dict(hover=[.598076211,4.964101615,0])),
 'arc_hover_endpoint':case(ARC['points'][1],(-8,6,0),None,dict(hover=[-8,6,0])),
 'arc_other_target':case(ARC['points'][1],target=[-6,8,0]),
})
PILOT=list(CASES)
if __package__:from .line_tangent_arcs_ends import ENDS
else:from line_tangent_arcs_ends import ENDS
CASES.update(ENDS)
if __package__:from .line_tangent_arcs_controls import CONTROLS
else:from line_tangent_arcs_controls import CONTROLS
CASES.update(CONTROLS)
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_tangent_arcs',id=k,case=k)for k in (PILOT if names is None else names)])
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='line_tangent_arcs'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed tangent arc recipe')
