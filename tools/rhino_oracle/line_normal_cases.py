"""Fixed curve/surface Normal inputs, independent of native output geometry."""
from copy import deepcopy
CIRCLE=dict(type='circle',center=[0,0,0],radius=5,x_axis=[1,0,0],normal=[0,0,1])
ARC=dict(type='arc',points=[[5,0,0],[3.5355339059327378,3.5355339059327378,0],[0,5,0]])
def quadratic(points):return dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1)for p in points],knots=[0,0,0,1,1,1])
QUAD=quadratic([[0,0,0],[4,6,0],[10,0,0]])
SPATIAL=quadratic([[0,0,0],[4,6,3],[10,0,2]])
def surface(points):return dict(type='surface',degree_u=1,degree_v=1,control_point_count_u=2,control_point_count_v=2,control_points=[dict(point=p,weight=1)for p in points],knots_u=[0,0,1,1],knots_v=[0,0,1,1])
PLANE=surface([[0,0,0],[0,10,0],[10,0,0],[10,10,0]])
CURVED=surface([[0,0,0],[0,10,0],[10,0,0],[10,10,4]])
def case(source,pick,base,*tail,**options):
 return dict(curves=[source],actions=(['BothSides']if options.get('both')else[])+['Normal',dict(curve=0,pick=pick),base]+list(tail))
CASES={
 'circle_point':case(CIRCLE,[4.99,.2,0],[3,4,0],[7,6,2]),
 'circle_offbase':case(CIRCLE,[4.99,.2,0],[4,4,3],[8,7,5]),
 'circle_both':case(CIRCLE,[4.99,.2,0],[3,4,0],[7,6,2],both=True),
 'circle_length':case(CIRCLE,[4.99,.2,0],[3,4,0],5,[8,9,3]),
 'arc_point':case(ARC,[3.5355339059327378,3.5355339059327378,0],[3,4,0],[7,6,2]),
 'quadratic_point':case(QUAD,[4.5,3,0],[4.5,3,0],[9,8,4]),
 'spatial_curve':case(SPATIAL,[4.5,3,2],[4.5,3,2],[9,8,6]),
 'plane_point':case(PLANE,[5,5,0],[4,3,0],[8,9,10]),
 'plane_offbase':case(PLANE,[5,5,0],[4,3,2],[8,9,10]),
 'curved_surface':case(CURVED,[5,5,1],[5,5,1],[8,7,6]),
 'plane_length':case(PLANE,[5,5,0],[4,3,0],5,[8,9,10]),
 'line_cancel':case(dict(type='line',start=[0,0,0],end=[10,0,0]),[5,0,0],[5,0,0],'Cancel'),
 'polyline_cancel':case(dict(type='polyline',vertices=[[0,0,0],[5,0,0],[5,5,0]]),[3,0,0],[3,0,0],'Cancel'),
 'cancel_reference':dict(curves=[CIRCLE],actions=['Normal',dict(curve=0,pick=[4.99,.2,0]),'Cancel']),
 'cancel_base':case(CIRCLE,[4.99,.2,0],[3,4,0],'Cancel'),
 'cancel_empty':dict(actions=['Normal','Cancel']),
}
PILOT=list(CASES)
if __package__:from .line_normal_holdouts import HOLDOUTS
else:from line_normal_holdouts import HOLDOUTS
CASES.update(HOLDOUTS)
if __package__:from .line_normal_final import FINAL
else:from line_normal_final import FINAL
CASES.update(FINAL)
if __package__:from .line_normal_brep import BREP
else:from line_normal_brep import BREP
CASES.update(BREP)
def request(names=None):
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_normal',id=n,case=n)for n in (PILOT if names is None else names)])
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='line_normal'or not isinstance(op.get('case'),str)or op['case']not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed Line Normal recipe')
def recipe(name):return deepcopy(CASES[name])
