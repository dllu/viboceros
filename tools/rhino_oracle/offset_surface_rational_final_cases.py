"""Final authored inputs after the rational patch and common-basis repair."""
import copy
if __package__:
    from .offset_surface_loose_cases import surface
    from .offset_surface_rational_cases import DOUBLE, rational
    from .offset_surface_tolerance_cases import BASE
else:
    from offset_surface_loose_cases import surface
    from offset_surface_rational_cases import DOUBLE, rational
    from offset_surface_tolerance_cases import BASE

CUBIC = surface([[[x,y,.04*i*i+.03*i*j-.05*j*j]
    for i,x in enumerate([0.,.6,1.1,2.2,3.4,5.2])]
    for j,y in enumerate([0.,.8,2.1,3.5])], degree=(3,3),
    knots=([-4.,-4.,-4.,-4.,-1.5,-1.5,3.,3.,3.,3.],
           [6.,6.,6.,6.,11.,11.,11.,11.]),
    weights=[[1.,.95,1.2,1.1,.85,1.05],[1.1,.9,1.25,.95,1.15,1.],
             [.95,1.15,.9,1.2,1.05,1.1],[1.05,.85,1.1,.95,1.2,1.]])
VARIED = copy.deepcopy(DOUBLE)
for index,control in enumerate(VARIED['control_points']):
    i,j=index%4,index//4
    control['point'][2] += .02*(i-j)
    control['weight'] *= 1.+.05*(i-1)

RATIONAL_FINAL = {
    'offset_rf_cubic_postselect': dict(rational([], CUBIC), preselected=False,
        actions=[{'curve':0},'Enter']+BASE+[.2,'Tolerance',.00001,'Enter']),
    'offset_rf_two_axes_both': rational([-.15,'BothSides=Yes','Tolerance',.0001,'Enter'], VARIED),
}
