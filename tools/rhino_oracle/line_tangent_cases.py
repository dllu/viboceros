"""Prescribed tangent-line workflows, independent of native output geometry."""
from copy import deepcopy
LINE=dict(type='line',start=[0,0,0],end=[10,0,0])
CIRCLE=dict(type='circle',center=[0,0,0],radius=5,x_axis=[1,0,0],normal=[0,0,1])
OTHER=dict(type='circle',center=[12,0,0],radius=2,x_axis=[1,0,0],normal=[0,0,1])
ARC=dict(type='arc',points=[[5,0,0],[3.5355339059327378,3.5355339059327378,0],[0,5,0]])
def quadratic(points):return dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1)for p in points],knots=[0,0,0,1,1,1])
QUAD=quadratic([[0,0,0],[4,6,0],[10,0,0]])
SPATIAL=quadratic([[0,0,0],[4,6,3],[10,0,2]])
def case(source,pick,*tail,**options):return dict(curves=[source],actions=(['BothSides']if options.get('both')else[])+['Tangent',dict(curve=0,pick=pick)]+list(tail))
def two(a,pa,b,pb):return dict(curves=[a,b],actions=['Tangent','2Curves',dict(curve=0,pick=pa),dict(curve=1,pick=pb)])
CASES={
 'line_tangent':case(LINE,[5,0,0],[7,4,3]),
 'circle_near':case(CIRCLE,[4.99,.2,0],[8,6,0]),
 'circle_far':case(CIRCLE,[-4.99,.2,0],[8,6,0]),
 'circle_spatial_end':case(CIRCLE,[4.99,.2,0],[8,6,2]),
 'circle_inside':case(CIRCLE,[4.99,.2,0],[1,1,2]),
 'arc_tangent':case(ARC,[3.5355339059327378,3.5355339059327378,0],[8,6,2]),
 'quadratic_tangent':case(QUAD,[4.5,3,0],[9,8,4]),
 'spatial_tangent':case(SPATIAL,[4.5,3,2],[9,8,6]),
 'fixed_tangent':case(CIRCLE,[4.99,.2,0],'FromFirstPoint',[8,6,2]),
 'oncurve_tangent':dict(curves=[CIRCLE],actions=['Tangent','PointOnCurve',dict(curve=0,pick=[4.99,.2,0]),[3,4,0],[8,6,2]]),
 'tangent_both':case(CIRCLE,[4.99,.2,0],[8,6,2],both=True),
 'numeric_then_point':case(CIRCLE,[4.99,.2,0],5,[8,6,2]),
 'numeric_only':case(CIRCLE,[4.99,.2,0],5),
 'fixed_numeric':case(CIRCLE,[4.99,.2,0],'FromFirstPoint',5),
 'point_free':dict(curves=[CIRCLE],actions=['Tangent','Point',[8,6,2],[1,1,3]]),
 'point_to_curve':dict(curves=[CIRCLE],actions=['Tangent','Point',[8,6,2],dict(curve=0,pick=[4.99,.2,0])]),
 'point_to_far_curve':dict(curves=[CIRCLE],actions=['Tangent','Point',[8,6,2],dict(curve=0,pick=[-4.99,.2,0])]),
 'two_circles_outer':two(CIRCLE,[0,5,0],OTHER,[12,2,0]),
 'two_circles_inner':two(CIRCLE,[0,5,0],OTHER,[12,-2,0]),
 'two_skew_lines':two(LINE,[5,0,0],dict(type='line',start=[2,-3,5],end=[2,9,5]),[2,6,5]),
 'cancel_picked':case(CIRCLE,[4.99,.2,0],'Cancel'),
 'cancel_empty':dict(actions=['Tangent','Cancel']),
}
PILOT=list(CASES)
if __package__:from .line_tangent_holdouts import HOLDOUTS
else:from line_tangent_holdouts import HOLDOUTS
CASES.update(HOLDOUTS)
if __package__:from .line_tangent_final import FINAL
else:from line_tangent_final import FINAL
CASES.update(FINAL)
if __package__:from .line_tangent_completion import COMPLETION
else:from line_tangent_completion import COMPLETION
CASES.update(COMPLETION)
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_tangent',id=n,case=n)for n in(PILOT if names is None else names)])
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='line_tangent'or not isinstance(op.get('case'),str)or op['case']not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed Line Tangent recipe')
def recipe(name):return deepcopy(CASES[name])
