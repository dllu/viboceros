"""Prescribed Line reference picks, independent of native output geometry."""
from copy import deepcopy
LINE=dict(type='line',start=[0,0,0],end=[10,0,0])
ARC=dict(type='arc',points=[[5,0,0],[3.5355339059327378,3.5355339059327378,0],[0,5,0]])
NURBS=dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=1)for p in [[0,0,0],[2,3,0],[7,4,0],[10,0,0]]],knots=[0,0,0,0,1,1,1,1])
POLYLINE=dict(type='polyline',vertices=[[0,0,0],[4,0,0],[4,3,0]])
def case(source,pick,*tail,**options):
 return dict(curves=[source],actions=(['BothSides']if options.get('both')else[])+['Extension',dict(curve=0,pick=pick)]+list(tail))
CASES={
 'line_end':case(LINE,[9,0,0],[14,6,5]),
 'line_start':case(LINE,[1,0,0],[-4,6,5]),
 'line_length':case(LINE,[9,0,0],5,[15,5,0]),
 'line_increment':case(LINE,[9,0,0],-5,[18,5,0]),
 'line_both':case(LINE,[9,0,0],[14,6,5],both=True),
 'arc_end':case(ARC,[.2,4.99,0],[-4,8,0]),
 'arc_start':case(ARC,[4.99,.2,0],[6,-4,0]),
 'nurbs_end':case(NURBS,[9.5,.6,0],[13,-4,0]),
 'polyline_end':case(POLYLINE,[4,2.8,0],[7,9,2]),
 'closed_cancel':case(dict(type='circle',center=[0,0,0],radius=5,x_axis=[1,0,0],normal=[0,0,1]),[4.99,.2,0],'Cancel'),
 'cancel_empty':dict(actions=['Extension','Cancel']),
 'cancel_selected':case(LINE,[9,0,0],'Cancel'),
}
PILOT=list(CASES)
if __package__:from .line_reference_holdouts import HOLDOUTS
else:from line_reference_holdouts import HOLDOUTS
CASES.update(HOLDOUTS)
if __package__:from .line_reference_completion import COMPLETION
else:from line_reference_completion import COMPLETION
CASES.update(COMPLETION)
if __package__:from .line_reference_final import FINAL
else:from line_reference_final import FINAL
CASES.update(FINAL)
def request(names=None):
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_reference',id=n,case=n)for n in (PILOT if names is None else names)])
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='line_reference'or not isinstance(op.get('case'),str)or op['case']not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed Line reference recipe')
def recipe(name):return deepcopy(CASES[name])
