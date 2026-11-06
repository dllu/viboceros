"""Clean constrained-base, boundary, cursor and signed-length holdouts."""
if __package__:
 from .line_perpendicular_cases import case,LINE,CIRCLE,SPATIAL
 from .line_perpendicular_holdouts import oncurve,two,SECOND
else:
 from line_perpendicular_cases import case,LINE,CIRCLE,SPATIAL
 from line_perpendicular_holdouts import oncurve,two,SECOND
WEIGHTED=dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=w)for p,w in zip([[0,0,0],[1,5,0],[5,4,0],[8,0,0]],[1,.2,2,1])],knots=[0,0,0,0,1,1,1,1])
FINAL={
 'oncurve_circle_clean':oncurve(CIRCLE,[4.99,.2,0],[3,4,0],[8,6,2]),
 'oncurve_signed_length':oncurve(CIRCLE,[4.99,.2,0],[3,4,0],-5,[8,6,2]),
 'oncurve_spatial_clean':oncurve(SPATIAL,[4.5,3,2],[4.5,3,2],[9,8,6]),
 'circle_signed_length':case(CIRCLE,[4.99,.2,0],-5,[8,6,2]),
 'numeric_enter_cancel':case(LINE,[5,0,0],5,'Enter'),
 'two_both':dict(two(LINE,[5,0,0],SECOND,[2,6,5]),actions=['BothSides']+two(LINE,[5,0,0],SECOND,[2,6,5])['actions']),
 'two_intersecting':two(LINE,[5,0,0],dict(type='line',start=[2,-3,0],end=[2,9,0]),[2,6,0]),
 'polyline_sliding':case(dict(type='polyline',vertices=[[0,0,0],[5,0,0],[5,5,0]]),[3,0,0],[7,4,3]),
 'weighted_sliding':case(WEIGHTED,[.05,.25,0],[2,3,5]),
 'planar_mouse_sliding':dict(case(SPATIAL,[4.5,3,2],dict(click=[9,8,6])),planar=True),
 'planar_mouse_fixed':dict(case(SPATIAL,[4.5,3,2],'FromFirstPoint',dict(click=[9,8,6])),planar=True),
 'mouse_spatial_sliding':case(SPATIAL,[4.5,3,2],dict(click=[9,8,6])),
 'point_line_outside':dict(curves=[LINE],actions=['Perpendicular','Point',[15,4,3],dict(curve=0,pick=[9,0,0])]),
 'perp_cplane':dict(case(CIRCLE,[4.99,.2,0],[8,6,2]),construction_plane=[[10,20,30],[0,1,0],[0,0,1]]),
 'redo_empty_cancel':dict(actions=['Perpendicular','Cancel'],pending_redo=True),
}
