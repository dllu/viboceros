"""Vertical planar guides and an exact polyline junction."""
if __package__:from .line_perpendicular_cases import case
else:from line_perpendicular_cases import case
VERTICAL=dict(type='line',start=[1,2,0],end=[1,2,10])
BOUNDARIES={
 'vertical_planar':dict(case(VERTICAL,[1,2,5],[9,8,6]),planar=True,view='Front'),
 'vertical_spatial':dict(case(VERTICAL,[1,2,5],[9,8,6]),view='Front'),
 'polyline_fixed_vertex':dict(curves=[dict(type='polyline',vertices=[[0,0,0],[5,0,0],[5,5,0]])],actions=['Perpendicular','PointOnCurve',dict(curve=0,pick=[3,0,0]),[5,0,0],[7,4,3]]),
}
