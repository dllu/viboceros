"""Public one-knot removal on measured C0 cubic pieces, before Loft joining."""
import copy
import json
from .capture_family import HERE, capture


def request():
    observed = json.loads((HERE / 'observations/loft_refit_multispan.json').read_text())
    surface = next(r['value']['surfaces'][0] for r in observed['results'] if
                   r['id'] == 'loft-refit-multispan-angle-0.25-plain-0.001')
    source = dict(degree=3, knots=[0.] * 4 + [.25] * 3 + [.5] * 3 + [1.] * 4,
                  control_points=copy.deepcopy(surface['control_points'][::surface['control_count'][0]]))
    operations = []
    for scale in [1., 1000.]:
        curve = copy.deepcopy(source)
        for cp in curve['control_points']:
            cp['point'] = [x * scale for x in cp['point']]
        for parameter in [.25, .5]:
            operations.append(dict(op='curve_remove_knot_geometry',
                id='loft-refit-join-knot-{}-{}'.format(scale, parameter),
                curve=copy.deepcopy(curve), parameter=parameter))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_join_knots', request(), ['generate_loft_refit_join_knots.py',
        'observations/loft_refit_multispan.json', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleLoftRefitJoinKnots20261026')
