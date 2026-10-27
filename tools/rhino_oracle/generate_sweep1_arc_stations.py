"""Independent analytic/rational arc station holdouts for the public Sweep1 command."""
import math
from .capture_family import capture


def request():
    operations = []
    blocks = [
        ('quarter', 3., 90., [0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [.2, .35, .5, .65, .8], 0.),
        ('oriented', .75, 100., [7., -3., 11.], [0., 1., 0.], [0., 0., 1.], [.2, .7], .2),
        ('reversed', 1.25, 90., [0., 0., 0.], [0., 1., 0.], [1., 0., 0.], [.2, .8], -.3),
    ]
    for name, radius, degrees, center, x_axis, y_axis, fractions, normal_offset in blocks:
        sweep = math.radians(degrees)
        normal = [x_axis[1]*y_axis[2]-x_axis[2]*y_axis[1],
                  x_axis[2]*y_axis[0]-x_axis[0]*y_axis[2],
                  x_axis[0]*y_axis[1]-x_axis[1]*y_axis[0]]
        def point(angle, r=radius, height=0.):
            return [center[i] + r*(math.cos(angle)*x_axis[i]+math.sin(angle)*y_axis[i])
                    + height*normal[i] for i in range(3)]
        analytic = dict(type='arc', points=[point(0.), point(sweep/2), point(sweep)])
        spans = math.ceil(degrees/90.)
        controls = [dict(point=point(0.), weight=1.)]
        for i in range(spans):
            half = sweep/(2*spans)
            controls.extend([dict(point=point((i+.5)*sweep/spans, radius/math.cos(half)), weight=math.cos(half)),
                             dict(point=point((i+1)*sweep/spans), weight=1.)])
        knots = [0.]*3 + [radius*sweep*i/spans for i in range(1, spans) for _ in range(2)] + [radius*sweep]*3
        rational = dict(type='nurbs', degree=2, control_points=controls, knots=knots)
        for fraction in fractions:
            section = dict(type='line', start=point(sweep*fraction),
                           end=point(sweep*fraction, 1.4*radius, normal_offset*radius))
            for representation, rail in [('analytic', analytic), ('rational', rational)]:
                for refit in [False, True]:
                    operations.append(dict(op='sweep1', id='arc-station-{}-{}-{}-{}'.format(
                        name, fraction, representation, 'refit' if refit else 'basis'),
                        rail=rail, sections=[section], command=True, refit_rail=refit,
                        inspect_definition=True, station_fraction=fraction))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10), operations=operations)


if __name__ == '__main__':
    capture('sweep1_arc_stations', request(),
            ['generate_sweep1_arc_stations.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSweep1ArcStations20261027')
