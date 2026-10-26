"""Compare native scalar clocks on the intervals used by adaptive fitting."""
from .capture_family import capture


def request():
    operations = []
    for reverse in [False, True]:
        for clock in [.2, 1. / 3., .5, .8]:
            points = [[0., 0., 0.], [1., 0., 0.], [3., 2e-5, 0.]]
            if reverse:
                points.reverse()
            def parameter(x):
                if reverse:
                    return clock * (3. - x) / 2. if x >= 1. else clock + (1. - clock) * (1. - x)
                return clock * x if x <= 1. else clock + (1. - clock) * (x - 1.) / 2.
            for a, b in [[.75, 1.5], [.9375, 1.125], [.9375, 1.03125]]:
                for shifted in [False, True]:
                    chart = (lambda t: 100. + 7. * t) if shifted else (lambda t: t)
                    interval = sorted([chart(parameter(a)), chart(parameter(b))])
                    for accuracy in [1e-4, 1e-2]:
                        operations.append(dict(op='curve_length_parameters',
                            id='interval-clock-{}-{}-{}-{}-{}-{}'.format(int(reverse), clock, a, b, int(shifted), accuracy),
                            curve=dict(degree=1, knots=[chart(t) for t in [0., 0., clock, 1., 1.]],
                                control_points=[dict(point=p[:], weight=1.) for p in points]),
                            subdomain=interval, fractional_tolerance=accuracy,
                            fractions=[0., .125, .25, .375, .5, .625, .75, .875, 1., .5, .25]))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('interval_length_clocks', request(),
        ['generate_interval_length_clocks.py', 'loft_scalar_clock_probe.py',
         'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleIntervalLengthClocks20261026')
