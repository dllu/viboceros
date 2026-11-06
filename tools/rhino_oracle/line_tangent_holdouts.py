"""Coplanar branches, fixed lengths and an exact spatial tangent."""
if __package__:from .line_tangent_cases import case,CIRCLE,LINE,ARC,QUAD,SPATIAL
else:from line_tangent_cases import case,CIRCLE,LINE,ARC,QUAD,SPATIAL
HOLDOUTS={
 'circle_pick_top':case(CIRCLE,[.2,4.99,0],[8,6,0]),
 'circle_pick_bottom':case(CIRCLE,[.2,-4.99,0],[8,6,0]),
 'circle_end_upper_left':case(CIRCLE,[4.99,.2,0],[-8,6,0]),
 'circle_end_lower_right':case(CIRCLE,[4.99,.2,0],[8,-6,0]),
 'circle_end_lower_left':case(CIRCLE,[4.99,.2,0],[-8,-6,0]),
 'circle_hover_upper':case(CIRCLE,[4.99,.2,0],dict(hover=[-.6,4.96,0]),[8,6,0]),
 'circle_hover_lower':case(CIRCLE,[4.99,.2,0],dict(hover=[4.6,-1.96,0]),[8,6,0]),
 'circle_mouse':case(CIRCLE,[4.99,.2,0],dict(click=[8,6,0])),
 'arc_planar_tangent':case(ARC,[3.5355339059327378,3.5355339059327378,0],[-8,6,0]),
 'quadratic_planar_tangent':case(QUAD,[4.5,3,0],[9,8,0]),
 'spatial_exact_tangent':case(SPATIAL,[4.5,3,2],[8.32,4.08,3.16]),
 'line_collinear_tangent':case(LINE,[5,0,0],[12,0,0]),
 'numeric_planar':case(CIRCLE,[4.99,.2,0],5,[8,6,0]),
 'numeric_planar_negative':case(CIRCLE,[4.99,.2,0],-5,[8,6,0]),
 'fixed_numeric_point':case(CIRCLE,[4.99,.2,0],'FromFirstPoint',5,[8,6,2]),
 'enter_fixed':case(CIRCLE,[4.99,.2,0],'Enter',[8,6,2]),
 'both_planar':case(CIRCLE,[4.99,.2,0],[8,6,0],both=True),
 'point_to_planar_curve':dict(curves=[CIRCLE],actions=['Tangent','Point',[8,6,0],dict(curve=0,pick=[4.99,.2,0])]),
 'point_to_far_planar_curve':dict(curves=[CIRCLE],actions=['Tangent','Point',[8,6,0],dict(curve=0,pick=[-4.99,.2,0])]),
 'point_end_planar':case(CIRCLE,[4.99,.2,0],'Point',[8,6,0]),
}
