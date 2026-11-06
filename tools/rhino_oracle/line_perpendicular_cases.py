"""Prescribed Line Perpendicular inputs, independent of observed output."""
from copy import deepcopy
LINE=dict(type='line',start=[0,0,0],end=[10,0,0])
CIRCLE=dict(type='circle',center=[0,0,0],radius=5,x_axis=[1,0,0],normal=[0,0,1])
ARC=dict(type='arc',points=[[5,0,0],[3.5355339059327378,3.5355339059327378,0],[0,5,0]])
def quadratic(points):return dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1)for p in points],knots=[0,0,0,1,1,1])
QUAD=quadratic([[0,0,0],[4,6,0],[10,0,0]])
SPATIAL=quadratic([[0,0,0],[4,6,3],[10,0,2]])
def case(source,pick,*tail,**options):
 return dict(curves=[source],actions=(['BothSides']if options.get('both')else[])+['Perpendicular',dict(curve=0,pick=pick)]+list(tail))
CASES={
 'line_sliding':case(LINE,[5,0,0],[7,4,3]),
 'line_outside':case(LINE,[5,0,0],[15,4,3]),
 'circle_sliding':case(CIRCLE,[4.99,.2,0],[8,6,2]),
 'circle_reverse_pick':case(CIRCLE,[-4.99,.2,0],[8,6,2]),
 'arc_sliding':case(ARC,[3.5355339059327378,3.5355339059327378,0],[8,6,2]),
 'arc_outside':case(ARC,[3.5355339059327378,3.5355339059327378,0],[-7,6,2]),
 'quadratic_sliding':case(QUAD,[4.5,3,0],[9,8,4]),
 'spatial_sliding':case(SPATIAL,[4.5,3,2],[9,8,6]),
 'from_first_late':case(CIRCLE,[4.99,.2,0],'FromFirstPoint=Yes',[8,6,2]),
 'from_first_early':dict(curves=[CIRCLE],actions=['Perpendicular','FromFirstPoint=Yes',dict(curve=0,pick=[4.99,.2,0]),[8,6,2]]),
 'perp_both':case(LINE,[5,0,0],[7,4,3],both=True),
 'perp_length':case(LINE,[5,0,0],5,[7,4,3]),
 'perp_point':dict(curves=[CIRCLE],actions=['Perpendicular','Point',[3,2,4],[8,6,2]]),
 'two_lines':dict(curves=[LINE,dict(type='line',start=[2,3,5],end=[2,9,5])],actions=['Perpendicular','2Curves',dict(curve=0,pick=[5,0,0]),dict(curve=1,pick=[2,6,5])]),
 'cancel_picked':case(CIRCLE,[4.99,.2,0],'Cancel'),
 'cancel_empty':dict(actions=['Perpendicular','Cancel']),
}
PILOT=list(CASES)
if __package__:from .line_perpendicular_holdouts import HOLDOUTS
else:from line_perpendicular_holdouts import HOLDOUTS
CASES.update(HOLDOUTS)
if __package__:from .line_perpendicular_final import FINAL
else:from line_perpendicular_final import FINAL
CASES.update(FINAL)
if __package__:from .line_perpendicular_completion import COMPLETION
else:from line_perpendicular_completion import COMPLETION
CASES.update(COMPLETION)
if __package__:from .line_perpendicular_boundaries import BOUNDARIES
else:from line_perpendicular_boundaries import BOUNDARIES
CASES.update(BOUNDARIES)
def request(names=None):
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_perpendicular',id=n,case=n)for n in (PILOT if names is None else names)])
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='line_perpendicular'or not isinstance(op.get('case'),str)or op['case']not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed Line Perpendicular recipe')
def recipe(name):return deepcopy(CASES[name])
