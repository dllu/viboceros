"""Independent size, translation and interval holdouts for Global Sweep1."""
from copy import deepcopy
from .capture_family import capture
from .generate_sweep1_global_shapes import request as initial_request


def request():
    query = initial_request()
    original = deepcopy(query['operations'][0])
    operations = []
    for name, stations, sizes in [
        ('two-first', [0., 2.], [2., 1.]),
        ('two-last', [0., 3.], [1., 3.]),
        ('two-last-larger', [0., 3.], [1., 4.]),
        ('three-larger', [0., 2., 5.], [2., 1., 4.]),
        ('three-constant-first', [0., 2., 5.], [2., 2., 3.]),
    ]:
        op = deepcopy(original)
        op.update(id=name, parameters=stations,
                  rail=dict(type='line', start=[0., 0., 0.], end=[0., 0., stations[-1]]),
                  sections=[dict(type='line', start=[0., 0., z], end=[x, 0., z])
                            for x, z in zip(sizes, stations)])
        operations.append(op)
    for name, scale, offset in [('scaled', 2., [0., 0., 0.]),
                                ('translated', 1., [1000., -400., 30.])]:
        op = deepcopy(original)
        op['id'] = name
        op['parameters'] = [s*scale for s in op['parameters']]
        for curve in [op['rail'], *op['sections']]:
            for key in ['start', 'end']:
                curve[key] = [x*scale+d for x,d in zip(curve[key], offset)]
        operations.append(op)
    query['operations'] = operations
    return query


if __name__ == '__main__':
    capture('sweep1_global_holdouts', request(), [
        'generate_sweep1_global_holdouts.py', 'generate_sweep1_global_shapes.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSweepGlobalHold20261027', timeout=240)
