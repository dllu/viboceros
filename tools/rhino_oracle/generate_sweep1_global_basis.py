"""Scalar basis, spacing and reversal probes for Global Sweep1 blending."""
from .capture_family import capture


def request(absolute=1e-7):
    operations = []
    for name, middle, widths in [
        ('base', 2., [2., 2., 2.]),
        ('basis-first', 2., [3., 2., 2.]),
        ('basis-middle', 2., [2., 3., 2.]),
        ('basis-last', 2., [2., 2., 3.]),
        ('combination', 2., [1.25, 3.5, 2.75]),
        ('spacing-one', 1., [2., 1., 3.]),
        ('spacing-half', 2.5, [2., 1., 3.]),
        ('spacing-four', 4., [2., 1., 3.]),
        ('reversed', 3., [3., 1., 2.]),
    ]:
        stations = [0., middle, 5.]
        operations.append(dict(op='sweep1', id=name,
            rail=dict(type='line', start=[0., 0., 0.], end=[0., 0., 5.]),
            sections=[dict(type='line', start=[0., 0., z], end=[x, 0., z])
                      for x,z in zip(widths, stations)],
            parameters=stations, blend=1, inspect_definition=True))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=absolute, relative=1e-12, angular=1e-10), operations=operations)


if __name__ == '__main__':
    capture('sweep1_global_basis', request(), [
        'generate_sweep1_global_basis.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepGlobalBasis20261027', timeout=240)
