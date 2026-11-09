"""Public Ribbon/Offset holdouts for mixed Sharp turns and Smooth blends."""
import copy
from .capture_family import capture
from .generate_ribbon import curve
from .generate_ribbon_polyline import request as polylines


def request():
    root = copy.deepcopy(polylines())
    root['operations'] = []
    for original in polylines()['operations']:
        if original['corner'] == 'Round':
            op = copy.deepcopy(original); op['corner'] = 'Smooth'
            op['id'] = op['id'].replace('round', 'smooth'); root['operations'].append(op)
    shapes = [
        ('left-left', [[0.,0.,0.],[5.,0.,0.],[7.,4.,0.],[3.,7.,0.]], None, 'nurbs'),
        ('acute-mixed', [[0.,0.,0.],[5.,0.,0.],[8.,4.,0.],[13.,4.,0.]], None, 'nurbs'),
        ('obtuse-mixed', [[0.,0.,0.],[5.,0.,0.],[2.,4.,0.],[7.,4.,0.]], None, 'nurbs'),
        ('right-left', [[0.,0.,0.],[5.,0.,0.],[5.,-4.,0.],[10.,-4.,0.]], None, 'nurbs'),
        ('four-zigzag', [[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[10.,4.,0.],[10.,8.,0.]], None, 'nurbs'),
        ('chart-mixed', [[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[10.,4.,0.]], [-4.,-4.,1.,3.,8.,8.], 'nurbs'),
        ('native-mixed', [[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[10.,4.,0.]], None, 'polyline'),
        ('polycurve-mixed', [[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[10.,4.,0.]], None, 'polycurve'),
    ]
    for name, points, knots, kind in shapes:
        for both in [False, True]:
            for command in ['ribbon_command', 'loose_offset_command']:
                root['operations'].append(dict(op=command, id=name+('-both' if both else '')+
                    ('-ribbon' if command == 'ribbon_command' else '-offset'), curve=curve(points, knots=knots),
                    distance=.35, side=[2.,-2.,0.], both_sides=both, corner='Sharp', loose=False,
                    in_cplane=False, output_layer='Current', native_kind=kind))
    for op in copy.deepcopy(root['operations'][:3]):
        op['op'] = 'loose_offset_command'; op['id'] += '-offset'; root['operations'].append(op)
    return root


if __name__ == '__main__':
    capture('ribbon_corner_correspondence', request(), ['generate_ribbon_corner_correspondence.py',
        'generate_ribbon_polyline.py', 'generate_ribbon.py', 'ribbon_probe.py', 'join_probe.py',
        'merge_edges_probe.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py'],
        'VibocerosOracleRibbonCornerCorrespondence20261109', timeout=1200)
