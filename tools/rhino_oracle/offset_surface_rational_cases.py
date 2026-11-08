"""Authored rational supports; no fitted native output is used as an input."""
import copy
if __package__:
    from .offset_surface_loose_cases import RATIONAL, surface
    from .offset_surface_tolerance_cases import case
else:
    from offset_surface_loose_cases import RATIONAL, surface
    from offset_surface_tolerance_cases import case


def rational(actions, source=RATIONAL, **kwargs):
    return case(actions, curves=[copy.deepcopy(source)], **kwargs)


SHIFTED = copy.deepcopy(RATIONAL)
SHIFTED['knots_u'] = [-8. + .3*(t-2.) for t in SHIFTED['knots_u']]
SHIFTED['knots_v'] = [17. + .4*(t+3.) for t in SHIFTED['knots_v']]
for control in SHIFTED['control_points']:
    x,y,z = control['point']
    control['point'] = [x+6., y-2., z+1.5]
REVERSED = dict(type='untrimmed_brep', surface=SHIFTED, reversed=True)

TRANSPOSED = copy.deepcopy(RATIONAL)
nu,nv = RATIONAL['control_point_count_u'],RATIONAL['control_point_count_v']
TRANSPOSED.update(control_point_count_u=nv, control_point_count_v=nu,
    degree_u=RATIONAL['degree_v'], degree_v=RATIONAL['degree_u'],
    knots_u=RATIONAL['knots_v'][:], knots_v=RATIONAL['knots_u'][:],
    control_points=[copy.deepcopy(RATIONAL['control_points'][i*nu+j])
        for j in range(nu) for i in range(nv)])
for control in TRANSPOSED['control_points']:
    x,y,z = control['point']
    control['point'] = [x+7., 2.-y, .8*z]

DOUBLE = surface([[[x,y,.08*i*i+.11*i*j-.05*j*j]
    for i,x in enumerate([0.,.8,2.4,4.5])]
    for j,y in enumerate([-1.,.3,1.9,4.])], degree=(2,2),
    knots=([2.,2.,2.,3.,7.,7.,7.],[10.,10.,10.,12.,15.,15.,15.]),
    weights=[[1.,1.15,.92,1.2],[.85,1.3,1.1,.9],
             [1.2,.95,1.4,1.05],[1.,1.1,.8,1.25]])

RATIONAL_PILOT = {
    'offset_rp_negative': rational([-.3,'Tolerance',.0001,'Enter']),
    'offset_rp_both': rational(['BothSides=Yes','Tolerance',.0001,'Enter']),
    'offset_rp_fine': rational(['Tolerance',.000001,'Enter']),
}
RATIONAL_HOLDOUTS = {
    'offset_rh_reversed_chart': rational([.25,'Tolerance',.0001,'Enter'], REVERSED),
    'offset_rh_v_break': rational([.35,'Tolerance',.0001,'Enter'], TRANSPOSED),
    'offset_rh_two_axes': rational([-.2,'Tolerance',.0001,'Enter'], DOUBLE),
    'offset_rh_cancel': rational(['Tolerance',.000001,'Cancel'], REVERSED),
}
