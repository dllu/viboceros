"""Bounded contacts, rejected-point retries and drafting precedence."""
if __package__:from .line_tangent_cases import case,two,CIRCLE,OTHER,LINE,ARC,QUAD,SPATIAL
else:from line_tangent_cases import case,two,CIRCLE,OTHER,LINE,ARC,QUAD,SPATIAL
FINAL={
 'circle_retry_spatial':case(CIRCLE,[4.99,.2,0],[8,6,2],[8,6,0]),
 'circle_retry_inside':case(CIRCLE,[4.99,.2,0],[1,1,0],[8,6,0]),
 'line_retry_offaxis':case(LINE,[5,0,0],[7,4,0],[12,0,0]),
 'quadratic_retry_spatial':case(QUAD,[4.5,3,0],[9,8,4],[9,8,0]),
 'spatial_retry':case(SPATIAL,[4.5,3,2],[9,8,6],[8.32,4.08,3.16]),
 'arc_right':case(ARC,[3.5355339059327378,3.5355339059327378,0],[8,6,0]),
 'arc_left_pick_end':case(ARC,[.2,4.99,0],[-8,6,0]),
 'arc_left_pick_start':case(ARC,[4.99,.2,0],[-8,6,0]),
 'arc_fixed':case(ARC,[3.5355339059327378,3.5355339059327378,0],'FromFirstPoint',[8,6,3]),
 'quadratic_outside':case(QUAD,[4.5,3,0],[15,-3,0]),
 'line_fixed':case(LINE,[5,0,0],'FromFirstPoint',[7,4,3]),
 'planar_circle':dict(case(CIRCLE,[4.99,.2,0],[8,6,2],[8,6,0]),planar=True),
 'planar_fixed':dict(case(CIRCLE,[4.99,.2,0],'FromFirstPoint',[8,6,2]),planar=True),
 'oncurve_both_length':dict(curves=[CIRCLE],actions=['Tangent','PointOnCurve',dict(curve=0,pick=[4.99,.2,0]),'BothSides',[3,4,0],-5,[8,6,2]]),
 'two_both':dict(curves=[CIRCLE,OTHER],actions=['BothSides','Tangent','2Curves',dict(curve=0,pick=[0,5,0]),dict(curve=1,pick=[12,2,0])]),
 'point_free_both':dict(curves=[CIRCLE],actions=['BothSides','Tangent','Point',[8,6,2],[1,1,3]]),
 'point_to_bottom_curve':dict(curves=[CIRCLE],actions=['Tangent','Point',[8,6,0],dict(curve=0,pick=[.2,-4.99,0])]),
 'point_end_fixed':case(CIRCLE,[4.99,.2,0],'Point','Enter',[8,6,0]),
 'cancel_length':case(CIRCLE,[4.99,.2,0],5,'Cancel'),
 'two_nested':two(CIRCLE,[0,5,0],dict(CIRCLE,radius=2),[0,2,0]),
}
