"""Open-domain tangent fallback controls, with float-only source recipes."""
if __package__:from .line_tangent_arcs import ARC,case
else:from line_tangent_arcs import ARC,case
LINE=dict(type='line',start=[0.,0.,0.],end=[10.,0.,0.])
def quadratic(points):return dict(type='nurbs',degree=2,knots=[0.,0.,0.,1.,1.,1.],control_points=[dict(point=p,weight=1.)for p in points])
QUAD=quadratic([[0.,0.,0.],[4.,6.,0.],[10.,0.,0.]])
ENDS={
 'end_arc_top':case([3.5355339059327378,3.5355339059327378,0.],[-8.,2.,0.]),
 'end_arc_start':case([3.5355339059327378,3.5355339059327378,0.],[2.,-8.,0.]),
 'end_arc_below':case([3.5355339059327378,3.5355339059327378,0.],[-8.,-6.,0.]),
 'end_arc_spatial':case([3.5355339059327378,3.5355339059327378,0.],[-8.,2.,3.]),
 'end_line_after':case([5.,0.,0.],[15.,4.,3.],LINE),
 'end_line_before':case([5.,0.,0.],[-5.,4.,3.],LINE),
 'end_quadratic_start':case([4.5,3.,0.],[-5.,0.,4.],QUAD),
 'end_quadratic_end':case([4.5,3.,0.],[15.,-10.,5.],QUAD),
 'end_spatial':case([4.5,3.,2.],[20.,-20.,15.],quadratic([[0.,0.,0.],[4.,6.,3.],[10.,0.,2.]])),
 'arc_scaled_corrected':case([1.7677669529663689,1.7677669529663689,0.],[-4.,3.,0.],dict(type='arc',points=[[.5*x for x in p]for p in ARC['points']])),
}
