"""Independent tensor range, rational weight, direction and tolerance cases."""
import copy
from .capture_family import capture


def surface(kind):
    degree=1 if kind=='linear'else 3
    knots=([0.,0.,.25,.5,.75,1.,1.]if degree==1 else[0.]*4+[.25]*2+[.5]*3+[1.]*4)
    n=len(knots)-degree-1
    controls=[]
    for j in range(n):
        for i in range(n):
            x=i/(n-1.);y=j/(n-1.)
            z=0. if kind=='plane'else .3*x*x+.2*y*y+.15*x*y+(.08 if i==n//2 and j==n//2 else 0.)
            controls.append(dict(point=[x,y,z],weight=1.+.1*((i+2*j)%3)if kind=='rational'else 1.))
    return dict(degree_u=degree,degree_v=degree,control_point_count_u=n,control_point_count_v=n,
        knots_u=knots,knots_v=knots,control_points=controls,domain_u=[2.,5.],domain_v=[-7.,-1.])


def request():
    operations=[]
    for kind in ['plane','polynomial','rational','linear']:
        s=surface(kind);degree=s['degree_u']
        for direction in ['u','v','both']:
            for limit in [0.,1e-9,.001,1.]:
                operations.append(dict(op='surface_knot_reduction',id='surface-reduce-'+kind+'-'+direction+'-'+str(limit),
                    surface=copy.deepcopy(s),direction=direction,minimum_multiplicity=1,maximum_multiplicity=degree+1,absolute_tolerance=limit))
    for minimum,maximum in [(0,3),(1,3),(2,4),(3,4)]:
        operations.append(dict(op='surface_knot_reduction',id='surface-reduce-range-'+str(minimum)+'-'+str(maximum),
            surface=surface('polynomial'),direction='both',minimum_multiplicity=minimum,maximum_multiplicity=maximum,absolute_tolerance=0.))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('surface_knot_reduction',request(),['generate_surface_knot_reduction.py','surface_knot_reduction_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],'VibocerosOracleSurfaceKnotReduction20261108',timeout=1200)
