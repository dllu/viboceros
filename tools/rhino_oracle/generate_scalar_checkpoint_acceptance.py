"""Exercise numerical scalar checkpoint acceptance near both endpoints."""
import copy
import json
from .capture_family import HERE, capture


def request():
    parents = json.loads((HERE / 'fixtures/loft_scalar_clock.json').read_text())
    names = ['quarter-0.1', 'cubic-0.1', 'quadratic-0.1',
             'polynomial_cubic-0.1', 'smooth_cubic-0.1', 'quintic-0.1', 'rational']
    fractions = sorted({0., .5, 1., *[i / 1024. for i in range(1, 41)],
                        *[1. - i / 1024. for i in range(1, 41)]})
    fractions += list(reversed(fractions)) + [1. / 64., .5, 1. / 64.]
    operations = []
    for name in names:
        original = next(o['curves'][0] for o in parents['operations']
                        if o['id'] == 'loft-scalar-clock-' + name)
        for variant in ['plain', 'shifted', 'reversed']:
            curve = copy.deepcopy(original)
            if variant == 'shifted':
                curve['knots'] = [-3. + 10. * t for t in curve['knots']]
            elif variant == 'reversed':
                first, last = curve['knots'][0], curve['knots'][-1]
                curve['knots'] = [first + last - t for t in reversed(curve['knots'])]
                curve['control_points'].reverse()
            for accuracy in [.005, .01, .02]:
                operations.append(dict(op='curve_length_parameters',
                    id='scalar-checkpoint-{}-{}-{}'.format(name, variant, accuracy),
                    curve=copy.deepcopy(curve), fractions=list(fractions),
                    fractional_tolerance=accuracy))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8),
                operations=operations)


if __name__ == '__main__':
    capture('scalar_checkpoint_acceptance', request(),
        ['generate_scalar_checkpoint_acceptance.py', 'fixtures/loft_scalar_clock.json',
         'loft_scalar_clock_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleScalarCheckpointAcceptance20261026')
