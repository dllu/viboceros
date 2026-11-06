"""Boundary length, Planar and zero-projection retry controls."""
if __package__:
 from .line_tangent_arcs import case
 from .line_tangent_arcs_ends import LINE,quadratic
else:
 from line_tangent_arcs import case
 from line_tangent_arcs_ends import LINE,quadratic
CONTROLS={
 'boundary_positive_length':case([5.,0.,0.],[15.,4.,3.],LINE,5.),
 'boundary_negative_length':case([5.,0.,0.],[15.,4.,3.],LINE,-5.),
 'boundary_bothsides':dict(curves=[LINE],actions=['BothSides','Tangent',dict(curve=0,pick=[5.,0.,0.]),[15.,4.,3.]]),
 'boundary_planar_spatial':dict(case([4.5,3.,2.],[20.,-20.,15.],quadratic([[0.,0.,0.],[4.,6.,3.],[10.,0.,2.]])),planar=True),
 'boundary_zero_retry':case([5.,0.,0.],[15.,4.,3.],LINE,[10.,4.,3.]),
}
