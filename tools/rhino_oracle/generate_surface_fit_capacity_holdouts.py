"""Independent degrees, unequal axes and a piecewise-linear ruled surface."""
import copy
from .capture_family import capture
from .generate_surface_fit_capacity import refined


def tensor(p,q):
    return dict(degree_u=p,degree_v=q,control_point_count_u=p+1,control_point_count_v=q+1,
        knots_u=[0.]*(p+1)+[1.]*(p+1),knots_v=[0.]*(q+1)+[1.]*(q+1),
        control_points=[dict(point=[i/float(p),j/float(q),.4*(i/float(p))**2+.3*(j/float(q))**2+.2*i*j/(p*q)],
                            weight=[1.,.6,1.7,1.2,.8][i]*[1.,1.4,.75,1.1][j])
                        for j in range(q+1)for i in range(p+1)],domain_u=[-2.,6.],domain_v=[3.,8.])


def cases():
    for p,q in [(2,1),(2,2),(3,2),(4,3)]:
        s=tensor(p,q)
        for label,u,v in [('base',(),()),('one',(.5,),(.5,)if q>1 else()),
                          ('multiple',(.5,)*p,(.5,)*q if q>1 else()),
                          ('distinct',(.25,.5,.75),(.25,.5,.75)if q>1 else())]:
            yield '{}-{}-{}'.format(p,q,label),refined(s,u,v),p,q,.005
    s=dict(degree_u=1,degree_v=1,control_point_count_u=4,control_point_count_v=2,
        knots_u=[0.,0.,.25,.75,1.,1.],knots_v=[0.,0.,1.,1.],
        control_points=[dict(point=[i/3.,j,z],weight=1.)for j in [0.,1.]for i,z in enumerate([0.,.5,-.2,0.])],
        domain_u=[-2.,6.],domain_v=[3.,8.])
    for label,u in [('base',()),('one',(.5,)),('distinct',(.125,.5,.875))]:
        yield 'linear-'+label,refined(s,u),1,1,.01


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='surface_fit',id='fit-capacity-holdout-'+name,surface=source,
                         degree_u=p,degree_v=q,fit_tolerance=limit)for name,source,p,q,limit in cases()])


if __name__=='__main__':
    capture('surface_fit_capacity_holdouts',request(),['generate_surface_fit_capacity_holdouts.py',
        'generate_surface_fit_capacity.py','generate_surface_fit_adaptive.py','generate_surface_knot_reduction.py',
        'generate_surface_knot_reduction_holdouts.py','surface_fit_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSurfaceFitCapacityHoldouts20261109',timeout=1200)
