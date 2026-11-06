"""Independent NURBS checks for the arc-length endpoint policy."""
if __package__:from .line_reference_cases import case
else:from line_reference_cases import case
LINEAR=dict(type='nurbs',degree=1,control_points=[dict(point=p,weight=1)for p in [[0,0,0],[100,0,0],[100,1,0],[101,1,0]]],knots=[0,0,1,2,3,3])
CUBIC=dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=1)for p in [[0,0,0],[200,0,0],[100,1,0],[101,1,0]]],knots=[0,0,0,0,1,1,1,1])
FINAL={
 'linear_nurbs_arclength':case(LINEAR,[75,0,0],[110,5,0]),
 'cubic_nurbs_arclength':case(CUBIC,[87.208,.104,0],[110,5,0]),
}
