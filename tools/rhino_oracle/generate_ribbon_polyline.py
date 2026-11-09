"""Ordinary Ribbon corner holdouts, with every option assigned explicitly."""
from .capture_family import capture
from .generate_ribbon import curve


def request():
    cases = [
        ('right', [[0., 0., 0.], [5., 0., 0.], [5., 4., 0.]], [7., -2., 0.]),
        ('acute', [[0., 0., 0.], [5., 0., 0.], [8., 4., 0.]], [4., -2., 0.]),
        ('obtuse', [[0., 0., 0.], [5., 0., 0.], [2., 4., 0.]], [4., -2., 0.]),
        ('zigzag', [[0., 0., 0.], [5., 0., 0.], [5., 4., 0.], [10., 4., 0.]], [2., -2., 0.]),
        ('closed', [[0., 0., 0.], [5., 0., 0.], [5., 4., 0.], [0., 4., 0.], [0., 0., 0.]], [7., -2., 0.]),
    ]
    operations = []
    for name, points, side in cases:
        for corner in ['Sharp', 'Round', 'Chamfer', 'None']:
            operations.append(dict(op='ribbon_command', id=name+'-'+corner.lower(),
                curve=curve(points), distance=.5, side=side, both_sides=False,
                corner=corner, loose=False, in_cplane=False, output_layer='Current'))
    for name, points, side in cases:
        for corner in ['Sharp', 'Round', 'Chamfer', 'None']:
            operations.append(dict(op='ribbon_command', id=name+'-both-'+corner.lower(),
                curve=curve(points), distance=.5, side=side, both_sides=True,
                corner=corner, loose=False, in_cplane=False, output_layer='Current'))
    # Translated tilted plane, reversed traversal and a native PolylineCurve.
    for name, points, side, native in [
        ('tilted', [[2., 3., 4.], [7., 3., 4.], [7., 5.4, 7.2]], [4., 1.8, 2.4], 'nurbs'),
        ('reversed', [[5., 4., 0.], [5., 0., 0.], [0., 0., 0.]], [7., -2., 0.], 'nurbs'),
        ('native', cases[0][1], cases[0][2], 'polyline'),
    ]:
        for corner in ['Sharp', 'Round', 'Chamfer', 'None']:
            operations.append(dict(op='ribbon_command', id=name+'-'+corner.lower(),
                curve=curve(points), distance=.35, side=side, both_sides=False,
                corner=corner, loose=False, in_cplane=False, output_layer='Current', native_kind=native))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6), operations=operations)


if __name__ == '__main__':
    capture('ribbon_polyline', request(), ['generate_ribbon_polyline.py', 'generate_ribbon.py',
        'ribbon_probe.py', 'join_probe.py', 'merge_edges_probe.py', 'structural_sdk.py',
        'rhino_worker.py', 'client.py'], 'VibocerosOracleRibbonPolyline20261109', timeout=1200)
