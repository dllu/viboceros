"""Exact constrained bases, numeric inputs, Point and two-curve holdouts."""
if __package__:from .line_perpendicular_cases import case,LINE,CIRCLE,QUAD,SPATIAL
else:from line_perpendicular_cases import case,LINE,CIRCLE,QUAD,SPATIAL
SECOND=dict(type='line',start=[2,-3,5],end=[2,9,5])
UPPER=dict(type='line',start=[0,8,4],end=[10,8,4])
OTHER=dict(type='circle',center=[12,0,0],radius=2,x_axis=[1,0,0],normal=[0,0,1])
def two(a,pa,b,pb,*tail):return dict(curves=[a,b],actions=['Perpendicular','2Curves',dict(curve=0,pick=pa),dict(curve=1,pick=pb)]+list(tail))
def oncurve(a,pick,base,*tail):return dict(curves=[a],actions=['Perpendicular','PointOnCurve',dict(curve=0,pick=pick),base]+list(tail))
HOLDOUTS={
 'oncurve_sliding':oncurve(LINE,[5,0,0],[3,0,0],[7,4,3]),
 'oncurve_fixed':oncurve(CIRCLE,[4.99,.2,0],[3,4,0],'FromFirstPoint',[8,6,2]),
 'oncurve_fixed_length':oncurve(CIRCLE,[4.99,.2,0],[3,4,0],'FromFirstPoint',5,[8,6,2]),
 'line_fixed':case(LINE,[5,0,0],'FromFirstPoint',[7,4,3]),
 'circle_from_first_no':case(CIRCLE,[4.99,.2,0],'FromFirstPoint=No',[8,6,2]),
 'circle_numeric_lock':case(CIRCLE,[4.99,.2,0],5,[8,6,2]),
 'line_numeric_lock':case(LINE,[5,0,0],3,[7,4,3]),
 'line_numeric_increment':case(LINE,[5,0,0],-3,[7,4,3]),
 'point_end':case(CIRCLE,[4.99,.2,0],'Point',[8,6,2]),
 'point_to_curve':dict(curves=[CIRCLE],actions=['Perpendicular','Point',[8,6,2],dict(curve=0,pick=[4.99,.2,0])]),
 'point_to_far_curve':dict(curves=[CIRCLE],actions=['Perpendicular','Point',[8,6,2],dict(curve=0,pick=[-4.99,.2,0])]),
 'two_skew_valid':two(LINE,[5,0,0],SECOND,[2,6,5]),
 'two_parallel':two(LINE,[3,0,0],dict(type='line',start=[0,5,0],end=[10,5,0]),[7,5,0]),
 'two_circles_near':two(CIRCLE,[4.99,.2,0],OTHER,[10,.05,0]),
 'two_circles_far':two(CIRCLE,[-4.99,.2,0],OTHER,[14,.05,0]),
 'two_nurbs_line':two(QUAD,[4.5,3,0],UPPER,[6,8,4]),
 'two_spatial_line':two(SPATIAL,[4.5,3,2],UPPER,[6,8,4]),
 'spatial_fixed':case(SPATIAL,[4.5,3,2],'FromFirstPoint',[9,8,6]),
 'planar_spatial':dict(case(SPATIAL,[4.5,3,2],[9,8,6]),planar=True),
 'planar_spatial_fixed':dict(case(SPATIAL,[4.5,3,2],'FromFirstPoint',[9,8,6]),planar=True),
 'cancel_two_first':dict(curves=[LINE],actions=['Perpendicular','2Curves',dict(curve=0,pick=[5,0,0]),'Cancel']),
 'perp_redo_cancel':dict(case(CIRCLE,[4.99,.2,0],'Cancel'),pending_redo=True),
}
