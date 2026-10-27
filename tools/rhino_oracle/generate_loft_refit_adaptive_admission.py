"""Independent adaptive tolerances, command styles, mixed sections and point ends."""
import copy
import json
from .capture_family import HERE, capture


def request():
    old = json.loads((HERE / 'fixtures/loft_refit_coarse.json').read_text())
    operations = []
    for name, tolerance in [('quarter', .0007), ('cubic', .025),
                            ('smooth_cubic', .035), ('quintic', .075)]:
        source = next(op['curves'][0] for op in old['operations']
                      if op['id'] == 'loft-refit-coarse-' + name + '-0.1')
        curves = []
        for height in [0., 2., 4.]:
            curve = copy.deepcopy(source)
            for control in curve['control_points']:
                control['point'][2] += height
            curves.append(curve)
        original = dict(op='loft', curves=curves, closed=False,
            refit_tolerance=tolerance, inspect_inputs=True, sample_geometry=True)
        for style in ['normal', 'loose', 'tight', 'straight', 'uniform']:
            for command in [False, True]:
                op = copy.deepcopy(original)
                op.update(id='loft-refit-adaptive-admission-{}-{}-{}'.format(name, style, int(command)),
                          style=style, command=command)
                operations.append(op)
        for variant in ['mixed', 'point-ends']:
            op = copy.deepcopy(original)
            op.update(id='loft-refit-adaptive-admission-{}-{}'.format(name, variant), style='normal')
            if variant == 'mixed':
                controls = op['curves'][1]['control_points']
                op['curves'][1] = dict(degree=1, knots=[0., 0., 1., 1.],
                    control_points=[copy.deepcopy(controls[0]), copy.deepcopy(controls[-1])])
                for control in op['curves'][1]['control_points']:
                    control['weight'] = 1.
            else:
                op.update(start_point=[0., 0., -4.], end_point=[0., 0., 10.])
            operations.append(op)
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == '__main__':
    capture('loft_refit_adaptive_admission', request(), [
        'generate_loft_refit_adaptive_admission.py', 'fixtures/loft_refit_coarse.json',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitAdaptiveAdmission20261027')
