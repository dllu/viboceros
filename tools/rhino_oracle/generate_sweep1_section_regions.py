"""Unequal section regions through unsimplified SDK and actual Sweep1 commands."""
import copy
from .capture_family import capture


def request():
    first = [[0., 0., 0.], [1., 2., 0.], [3., 1., 0.]]
    second = [[0., 0., 5.], [1., 2., 5.], [2., -.3, 5.], [3., 1., 5.]]
    def nurbs(points, knots):
        return dict(type='nurbs', degree=1, knots=knots,
                    control_points=[dict(point=p, weight=1.) for p in points])
    def cubic(points):
        controls = [points[0]]
        for a, b in zip(points, points[1:]):
            controls.extend([[x + (y-x)/3. for x, y in zip(a, b)],
                             [y - (y-x)/3. for x, y in zip(a, b)], b])
        knots = [0.] * 4
        for i in range(1, len(points)-1):
            knots.extend([float(i)] * 3)
        knots.extend([float(len(points)-1)] * 4)
        return dict(type='nurbs', degree=3, knots=knots,
                    control_points=[dict(point=p, weight=1.) for p in controls])
    variants = {
        'unequal-corners': [dict(type='polyline', vertices=first), dict(type='polyline', vertices=second)],
        'unequal-domains': [nurbs(first, [10., 10., 12., 20., 20.]), nurbs(second, [-5., -5., -2., 3., 10., 10.])],
        'cubic-corners': [cubic(first), cubic(second)],
        'equal-corners': [dict(type='polyline', vertices=first), dict(type='polyline', vertices=[second[0], second[2], second[3]])],
    }
    operations = []
    for command in [False, True]:
        for name, sections in variants.items():
            operations.append(dict(op='sweep1', id=name + ('-command' if command else '-sdk'),
                command=command, rail=dict(type='line', start=[0., 0., 0.], end=[0., 0., 5.]),
                sections=copy.deepcopy(sections), parameters=[0., 5.], refit_rail=True, inspect_definition=True))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10), operations=operations)


if __name__ == '__main__':
    capture('sweep1_section_regions', request(), [
        'generate_sweep1_section_regions.py', 'sweep1_probe.py', 'rhino_worker.py',
        'client.py', 'structural_sdk.py'], 'VibocerosOracleSweepRegions20261028', timeout=300)
