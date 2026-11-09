"""Fit discovery: dense/redundant, polynomial/rational and requested degrees."""
from .capture_family import capture
from .generate_surface_knot_reduction import surface
from .generate_surface_knot_reduction_holdouts import surface as redundant


def request():
    operations = []
    for name, source in [('plane', surface('plane')), ('polynomial', surface('polynomial')),
                         ('rational', surface('rational')), ('linear', surface('linear')),
                         ('redundant', redundant(False, 3)), ('rational_redundant', redundant(True, 2))]:
        for degrees, limit in [((3, 3), 1e-7), ((3, 3), .01), ((3, 3), 1.),
                               ((2, 2), .01), ((5, 3), .01), ((1, 1), .01)]:
            operations.append(dict(op='surface_fit', id='fit-{}-{}-{}-{}'.format(name, *degrees, limit),
                                   surface=source, degree_u=degrees[0], degree_v=degrees[1], fit_tolerance=limit))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6), operations=operations)


if __name__ == '__main__':
    capture('surface_fit', request(), ['generate_surface_fit.py', 'surface_fit_probe.py',
            'generate_surface_knot_reduction.py', 'generate_surface_knot_reduction_holdouts.py',
            'structural_sdk.py', 'rhino_worker.py', 'client.py'],
            'VibocerosOracleSurfaceFit20261108', timeout=1200)
