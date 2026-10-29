"""Large-tolerance holdouts with tall, mixed and transformed profiles."""
import copy
import json
from pathlib import Path
from .capture_family import capture

HERE = Path(__file__).resolve().parent
FAMILY = 'sweep1_refit_large_holdouts'
INPUTS = 'fixtures/loft_refit_coarse.json'
CLOSED_INPUTS = 'fixtures/sweep1_refit_independent_profiles.json'


def request():
    base = json.loads((HERE / INPUTS).read_text())
    sources = {op['id'][len('loft-refit-coarse-'):-len('-100.0')]: op['curves'][0]
        for op in base['operations'] if op['refit_tolerance'] == 100.}
    independent = json.loads((HERE / CLOSED_INPUTS).read_text())
    closed = next(op['sections'][0] for op in independent['operations']
        if op['id'] == 'asymmetric-closed-cubic-0.002-count-1')
    tall = copy.deepcopy(sources['quadratic'])
    tall['control_points'][1]['point'] = [2.,100.,0.]
    transformed = copy.deepcopy(sources['quadratic'])
    for control in transformed['control_points']:
        x,y,z = control['point']
        control['point'] = [7.+100.*z,-3.+100.*x,5.+100.*y]
    gauged = copy.deepcopy(sources['quadratic'])
    gauged['knots'] = [-7.+20.*t for t in gauged['knots']]
    for control in gauged['control_points']:
        control['weight'] *= -7.
    operations = []
    def add(name, curves, tolerance):
        operations.append(dict(op='sweep_refit_sections',id=name,
            sections=copy.deepcopy(curves),parameters=[-7.,13.][:len(curves)],
            refit_tolerance=tolerance))
    def pair(source):
        second = copy.deepcopy(source)
        for control in second['control_points']:
            control['point'][2] += 5.
        return [source,second]
    for label, source, tolerances in [
        ('closed',closed,[100.]),('tall',tall,[4.,100.,200.]),
        ('scaled-frame',transformed,[400.]),('negative-gauge',gauged,[4.,100.])]:
        for tolerance in tolerances:
            for count in [1,2]:
                add('large-holdout-{}-{}-count-{}'.format(label,tolerance,count),pair(source)[:count],tolerance)
    add('large-holdout-mixed-open', [sources['quarter'],pair(sources['quadratic'])[1]],100.)
    add('large-holdout-mixed-closed', [sources['circle'],pair(closed)[1]],100.)
    return dict(protocol_version=1,iterations=1,tolerance=base['tolerance'],operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), ['generate_sweep1_refit_large_holdouts.py', INPUTS,CLOSED_INPUTS,
        'sweep_refit_sections_probe.py','geometry_validity_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleLargeRefitHoldouts20261029',timeout=300)
