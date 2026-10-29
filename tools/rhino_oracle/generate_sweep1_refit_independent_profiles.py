"""Independent asymmetric, spatial and relocated section-fitting references."""
import copy
import json
from pathlib import Path
from .capture_family import capture

HERE = Path(__file__).resolve().parent
FAMILY = 'sweep1_refit_independent_profiles'
INPUTS = 'fixtures/sweep1_refit_sections.json'


def request():
    base = json.loads((HERE / INPUTS).read_text())
    def polynomial(points):
        return dict(degree=len(points)-1, knots=[0.]*len(points)+[1.]*len(points),
                    control_points=[dict(point=p, weight=1.) for p in points])
    circle = copy.deepcopy(next(op['sections'][0] for op in base['operations']
        if op['id'] == 'nurbs-circle-1e-06-original-count-1'))
    for control in circle['control_points']:
        x, y, unused = control['point']
        control['point'] = [1.3 + 1.35*x, -.7 + 1.35*y, .4]
    sources = [
        ('asymmetric-quadratic', polynomial([[0.,0.,0.],[1.2,2.4,0.],[3.8,.3,0.]]), [.8e-6,1.2e-6]),
        ('spatial-quadratic', polynomial([[0.,0.,0.],[1.2,2.4,1.1],[3.8,.3,-.4]]), [.8e-6,1.2e-6]),
        ('translated-scaled-circle', circle, [.8e-6,1.2e-6]),
        ('asymmetric-closed-cubic', polynomial([[0.,0.,0.],[3.,1.,0.],[1.,3.,0.],[0.,0.,0.]]), [.002,.008]),
    ]
    operations = []
    for name, source, tolerances in sources:
        for tolerance in tolerances:
            second = copy.deepcopy(source)
            for control in second['control_points']:
                control['point'][2] += 5.
            for count in [1, 2]:
                operations.append(dict(op='sweep_refit_sections',
                    id='{}-{}-count-{}'.format(name,tolerance,count),
                    sections=copy.deepcopy([source,second][:count]),
                    parameters=[5.*i for i in range(count)], refit_tolerance=tolerance))
    return dict(protocol_version=1, iterations=1, tolerance=base['tolerance'], operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), [
        'generate_sweep1_refit_independent_profiles.py', INPUTS,
        'sweep_refit_sections_probe.py', 'geometry_validity_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleIndependentRefit20261029', timeout=300)
