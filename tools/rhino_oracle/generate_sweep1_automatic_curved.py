"""Public command outputs for automatic Sweep1 placement on curved rails."""
from math import sqrt
from .capture_family import capture


def request():
    arc = dict(type='arc', points=[[3., 0., 0.], [3. / sqrt(2), 3. / sqrt(2), 0.], [0., 3., 0.]])
    spatial = dict(type='nurbs', degree=3,
                   control_points=[dict(point=p, weight=1.) for p in
                                   [[0., 0., 0.], [2., 0., 4.], [3., 4., -2.], [5., 2., 3.]]],
                   knots=[0., 0., 0., 0., 1., 1., 1., 1.])
    line = lambda a, b: dict(type='line', start=a, end=b)
    arc_sections = [line([3., 0., 0.], [4., 0., 0.]),
                    line([3. / sqrt(2), 3. / sqrt(2), 0.], [4. / sqrt(2), 4. / sqrt(2), 0.]),
                    line([0., 3., 0.], [0., 4., 0.])]
    spatial_sections = [line([0., 0., 0.], [0., 1., 0.]),
                        line([2.5, 1.75, 1.125], [2.5, 1.75, 2.125]),
                        line([5., 2., 3.], [5., 3., 3.])]
    operations = []
    for name, rail, sections in [('arc', arc, arc_sections), ('spatial', spatial, spatial_sections)]:
        for placement, profiles in [('start', sections[:1]), ('interior', sections[1:2]),
                                    ('end', sections[2:]), ('reversed', sections[::-1])]:
            operations.append(dict(op='sweep1', id='automatic-' + name + '-' + placement,
                                   rail=rail, sections=profiles, command=True, refit_rail=False,
                                   inspect_definition=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10), operations=operations)


if __name__ == '__main__':
    capture('sweep1_automatic_curved', request(),
            ['generate_sweep1_automatic_curved.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSweep1AutomaticCurved20261027')
