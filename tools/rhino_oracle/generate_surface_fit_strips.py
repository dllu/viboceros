"""Directional strip fits isolate geometric error from tensor coupling."""
from .capture_family import capture
from .generate_surface_fit_adaptive import bezier, dense
from .generate_surface_knot_reduction import surface


def strip(source, swap=False):
    n=source['control_point_count_u'];points=source['control_points'][:n]
    controls=[dict(point=[p['point'][0],float(j),p['point'][2]],weight=p['weight']) for j in [0,1] for p in points]
    s=dict(degree_u=source['degree_u'],degree_v=1,control_point_count_u=n,control_point_count_v=2,
           knots_u=source['knots_u'],knots_v=[0.,0.,1.,1.],control_points=controls,
           domain_u=[2.,5.],domain_v=[-7.,-1.])
    if swap:
        for a,b in [('degree_u','degree_v'),('control_point_count_u','control_point_count_v'),
                    ('knots_u','knots_v'),('domain_u','domain_v')]:s[a],s[b]=s[b],s[a]
        s['control_points']=[controls[j*n+i] for i in range(n) for j in range(2)]
    return s


def request():
    operations=[]
    for name,source in [('rational_bezier',dense(bezier(True))),('polynomial',dense(surface('polynomial'))),
                        ('plane',dense(bezier(True,True)))]:
        for swap in [False,True]:
            s=strip(source,swap)
            for degree,limit in [(3,.05),(3,.01),(3,.001),(2,.01),(1,.01)]:
                operations.append(dict(op='surface_fit',id='fit-strip-{}-{}-{}-{}'.format(name,int(swap),degree,limit),
                                       surface=s,degree_u=1 if swap else degree,degree_v=degree if swap else 1,
                                       fit_tolerance=limit))
    # Planar images with separable and coupled rational parameterizations.
    for coupled in [False,True]:
        s=dense(bezier(True,True))
        if coupled:
            for i,p in enumerate(s['control_points']):p['weight']=1.+.25*((i+2*(i//s['control_point_count_u']))%3)
        for degree in [1,2,3]:
            operations.append(dict(op='surface_fit',id='fit-rational-plane-{}-{}'.format(int(coupled),degree),
                                   surface=s,degree_u=degree,degree_v=degree,fit_tolerance=.001))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('surface_fit_strips',request(),['generate_surface_fit_strips.py','generate_surface_fit_adaptive.py',
            'surface_fit_probe.py','generate_surface_knot_reduction.py','generate_surface_knot_reduction_holdouts.py',
            'structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOracleSurfaceFitStrips20261108',timeout=1200)
