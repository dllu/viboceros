"""Controlled ruled-strip changes and the public fit of their section curve."""
import copy

from .capture_family import capture
from .generate_surface_fit_adaptive import bezier, dense
from .generate_surface_fit_strips import strip


def request():
    source = strip(dense(bezier(True)))
    operations = []

    def surface(name, shape, limit):
        operations.append(dict(op='surface_fit_achieved', id=name, surface=shape,
                               degree_u=3, degree_v=shape['degree_v'], fit_tolerance=limit))

    for width in [.001, .1, 1., 10., 1000.]:
        for limit in [.001, .01, .03]:
            shape = copy.deepcopy(source)
            for p in shape['control_points']:
                p['point'][1] *= width
            surface('width-{}-{}'.format(width, limit), shape, limit)

    n = source['control_point_count_u']
    for degree in [2, 3]:
        for limit in [.001, .01, .03]:
            shape = copy.deepcopy(source)
            shape.update(degree_v=degree, control_point_count_v=degree+1,
                         knots_v=[0.]*(degree+1)+[1.]*(degree+1))
            shape['control_points'] = [dict(point=[p['point'][0], j/float(degree), p['point'][2]],
                                            weight=p['weight'])
                                       for j in range(degree+1) for p in source['control_points'][:n]]
            surface('transverse-degree-{}-{}'.format(degree, limit), shape, limit)

    for label, domain in [('wide', [-1e6, 1e6]), ('tiny', [1e-6, 2e-6])]:
        for limit in [.001, .01]:
            shape = copy.deepcopy(source)
            shape['domain_v'] = domain
            surface('domain-{}-{}'.format(label, limit), shape, limit)

    for scale in [.01, 100.]:
        for limit in [.001, .01]:
            shape = copy.deepcopy(source)
            for p in shape['control_points']:
                p['point'] = [x*scale for x in p['point']]
            surface('scale-{}-{}'.format(scale, limit), shape, limit*scale)

    curve = dict(degree=source['degree_u'], knots=source['knots_u'], domain=source['domain_u'],
                 control_points=source['control_points'][:n])
    for degree in [2, 3]:
        for limit in [.001, .01, .03]:
            operations.append(dict(op='curve_fit_geometry', id='curve-{}-{}'.format(degree, limit),
                                   curve=copy.deepcopy(curve), degree=degree, fit_tolerance=limit,
                                   angle_tolerance_radians=0.))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6), operations=operations)


if __name__ == '__main__':
    capture('surface_fit_covariance', request(), [
        'generate_surface_fit_covariance.py', 'generate_surface_fit_strips.py',
        'generate_surface_fit_adaptive.py', 'generate_surface_knot_reduction.py',
        'generate_surface_knot_reduction_holdouts.py', 'surface_fit_achieved_probe.py',
        'surface_fit_probe.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py'],
        'VibocerosOracleSurfaceFitCovariance20261109', timeout=1200)
