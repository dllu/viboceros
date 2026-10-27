"""Check whether a retraced line image can use an initial Loft cubic."""
from .capture_family import capture


def request():
    operations = []
    for name, xs in [('ordered', [0., .0625, .5, 1.]),
                     ('retraced', [0., 1., 1., 0., 0., 1.])]:
        degree = len(xs) - 1
        for tolerance in [1e-6, .001]:
            curves = [dict(degree=degree, knots=[0.] * (degree + 1) + [1.] * (degree + 1),
                control_points=[dict(point=[x, 0., z], weight=1.) for x in xs]) for z in [0., 2., 4.]]
            operations.append(dict(op='loft',
                id='loft-refit-projected-order-{}-{}'.format(name, tolerance),
                curves=curves, style='straight', closed=False,
                refit_tolerance=tolerance, inspect_inputs=True,
                sample_geometry=True, allow_empty=True))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-8), operations=operations)


if __name__ == '__main__':
    capture('loft_refit_projected_order', request(), [
        'generate_loft_refit_projected_order.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleLoftRefitProjectedOrder20261027')
