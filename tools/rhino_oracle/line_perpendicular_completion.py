"""Real Enter keys and verified Planar settings for cursor completion."""
if __package__:from .line_perpendicular_cases import case,LINE,CIRCLE,SPATIAL
else:from line_perpendicular_cases import case,LINE,CIRCLE,SPATIAL
COMPLETION={
 'enter_then_point':case(LINE,[5,0,0],'Enter',[7,4,3]),
 'enter_length_then_point':case(LINE,[5,0,0],3,'Enter',[7,4,3]),
 'fixed_enter_cancel':case(CIRCLE,[4.99,.2,0],'FromFirstPoint','Enter'),
 'verified_planar_mouse':dict(case(SPATIAL,[4.5,3,2],dict(click=[9,8,6])),planar=True),
 'verified_planar_fixed_mouse':dict(case(SPATIAL,[4.5,3,2],'FromFirstPoint',dict(click=[9,8,6])),planar=True),
 'verified_planar_typed':dict(case(SPATIAL,[4.5,3,2],[9,8,6]),planar=True),
 'verified_planar_fixed_typed':dict(case(SPATIAL,[4.5,3,2],'FromFirstPoint',[9,8,6]),planar=True),
 'point_override_mouse':dict(curves=[LINE],actions=['Perpendicular','Point',[15,4,3],'Point',dict(click=[9,0,0])]),
}
