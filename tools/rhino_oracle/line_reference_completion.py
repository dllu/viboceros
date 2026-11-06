"""Discriminate parameter/arc-length endpoint choice and weighted references."""
if __package__:from .line_reference_cases import case,LINE
else:from line_reference_cases import case,LINE
FIRST=dict(type='polyline',vertices=[[0,0,0],[100,0,0],[100,1,0],[101,1,0]])
LAST=dict(type='polyline',vertices=[[0,0,0],[1,0,0],[1,1,0],[101,1,0]])
JOINED=dict(type='polycurve',segments=[dict(type='line',start=a,end=b)for a,b in zip(FIRST['vertices'],FIRST['vertices'][1:])])
WEIGHTED=dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=w)for p,w in zip([[0,0,0],[1,5,0],[5,4,0],[8,0,0]],[1,.2,2,1])],knots=[0,0,0,0,1,1,1,1])
COMPLETION={
 'polyline_parameter_first':case(FIRST,[75,0,0],[110,5,0]),
 'polyline_parameter_last':case(LAST,[40,1,0],[-5,-5,0]),
 'polycurve_length_domain':case(JOINED,[75,0,0],[110,5,0]),
 'weighted_nurbs_end':case(WEIGHTED,[7.7,.4,0],[11,-4,0]),
 'weighted_nurbs_start':case(WEIGHTED,[.05,.25,0],[-1,-5,0]),
 'enter_length':case(LINE,[9,0,0],5,'Enter'),
 'enter_both_length':case(LINE,[9,0,0],5,'Enter',both=True),
 'zero_endpoint_cancel':case(LINE,[9,0,0],[10,0,0],'Cancel'),
}
