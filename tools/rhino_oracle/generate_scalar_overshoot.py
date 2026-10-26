"""Scalar inverse checkpoints near the start and after an overshooting step."""
import copy
import json
from .capture_family import HERE, capture


def request():
    query = json.loads((HERE / 'fixtures/loft_scalar_clock.json').read_text())
    sources = {op['id']: op['curves'][0] for op in query['operations']}
    fractions = [0., 1./1024., 1./512., 1./256., 1./128., 1./64.,
                 3./128., 1./32., 3./64., 1./16., .125, .375, .5, .875, 63./64., 1.]
    operations = []
    for name in ['quarter', 'cubic', 'quintic', 'rational']:
        key = 'loft-scalar-clock-' + name + ('' if name == 'rational' else '-0.1')
        for shifted in [False, True]:
            source = copy.deepcopy(sources[key])
            if shifted:
                source['knots'] = [-3. + 10.*t for t in source['knots']]
            for accuracy in [.002, .01, .02]:
                operations.append(dict(op='curve_length_parameters',
                    id='scalar-overshoot-{}-{}-{}'.format(name, 'shifted' if shifted else 'base', accuracy),
                    curve=copy.deepcopy(source), fractional_tolerance=accuracy,
                    fractions=fractions + list(reversed(fractions))))
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('scalar_overshoot', request(), ['generate_scalar_overshoot.py',
        'fixtures/loft_scalar_clock.json', 'loft_scalar_clock_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleScalarOvershoot20261025')
