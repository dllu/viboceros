"""Separate fitting policy from source degree, redundancy and representation."""
import copy
from .capture_family import capture
from .generate_surface_knot_reduction import surface
from .generate_surface_knot_reduction_holdouts import insert_line


def insert(source, axis, t):
    s = copy.deepcopy(source)
    nu, nv = s['control_point_count_u'], s['control_point_count_v']
    controls = s['control_points']
    key = 'knots_' + axis
    degree = s['degree_' + axis]
    lines = ([controls[j*nu:(j+1)*nu] for j in range(nv)] if axis == 'u' else
             [[controls[j*nu+i] for j in range(nv)] for i in range(nu)])
    following = [insert_line(line, s[key], degree, t) for line in lines]
    s[key] = following[0][1]
    if axis == 'u':
        s['control_point_count_u'] += 1
        s['control_points'] = [p for line, _ in following for p in line]
    else:
        s['control_point_count_v'] += 1
        s['control_points'] = [following[i][0][j] for j in range(nv+1) for i in range(nu)]
    return s


def dense(source):
    for axis in ['u', 'v']:
        for t in [.0625, .125, .375, .625, .75, .875, .9375]:
            source = insert(source, axis, t)
    return source


def bezier(rational=False, plane=False):
    controls = [dict(point=[i/3., j/3., 0. if plane else .3*i*(i-1)/6.+.2*j*(j-1)/6.+.15*i*j/9.],
                     weight=([1., .75, 1.5, 1.25][i]*[1., 2., .5, 1.][j]) if rational else 1.)
                for j in range(4) for i in range(4)]
    return dict(degree_u=3, degree_v=3, control_point_count_u=4, control_point_count_v=4,
                control_points=controls, knots_u=[0.]*4+[1.]*4, knots_v=[0.]*4+[1.]*4,
                domain_u=[2., 5.], domain_v=[-7., -1.])


def request():
    cases=[]
    families=[('polynomial', surface('polynomial')), ('rational', surface('rational')),
              ('bezier', bezier()), ('rational_bezier', bezier(True))]
    for name, source in families:
        for refined in [False, True]:
            s=dense(source) if refined else source
            for degree, limit in [(3, .05), (3, .01), (3, .001), (2, .01)]:
                cases.append(dict(op='surface_fit', id='fit-adaptive-{}-{}-{}-{}'.format(name,int(refined),degree,limit),
                                  surface=s, degree_u=degree, degree_v=degree, fit_tolerance=limit))
    for degree in [1, 2, 3, 5]:
        for n in [2, 4]:
            s=bezier(plane=True)
            if n==2:
                s.update(degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
                         knots_u=[0.,0.,1.,1.], knots_v=[0.,0.,1.,1.],
                         control_points=[dict(point=[i,j,0.],weight=1.) for j in [0.,1.] for i in [0.,1.]])
            cases.append(dict(op='surface_fit',id='fit-adaptive-small-plane-{}-{}'.format(n,degree),
                              surface=s,degree_u=degree,degree_v=degree,fit_tolerance=1e-7))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=cases)


if __name__=='__main__':
    capture('surface_fit_adaptive',request(),['generate_surface_fit_adaptive.py','surface_fit_probe.py',
            'generate_surface_knot_reduction.py','generate_surface_knot_reduction_holdouts.py',
            'structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOracleSurfaceFitAdaptive20261108',timeout=1200)
