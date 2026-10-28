"""Complete Sweep1 definitions distinguishing local and global profile blending."""
from .capture_family import capture


def request():
    operations = []
    recipes = [
        ('three', [0., 2., 5.], [(0., 2.), (0., 1.), (0., 3.)]),
        ('four', [0., 1., 3., 5.], [(0., 2.), (0., 1.), (0., 4.), (0., 3.)]),
        ('offset', [0., 2., 5.], [(0., 2.), (-1., 1.), (1., 4.)]),
    ]
    for name, stations, intervals in recipes:
        for producer in ['sdk', 'refit', 'retained']:
            operations.append(dict(
                op='sweep1', id=name+'-'+producer,
                rail=dict(type='line', start=[0., 0., 0.], end=[0., 0., 5.]),
                sections=[dict(type='line', start=[a, 0., z], end=[b, 0., z])
                          for z, (a, b) in zip(stations, intervals)],
                parameters=stations, blend=1, command=producer!='sdk',
                refit_rail=producer=='refit', inspect_definition=True,
            ))
    for producer in ['sdk', 'refit']:
        operation = dict(operations[0])
        operation.update(id='three-local-'+producer, blend=0,
                         command=producer!='sdk', refit_rail=producer=='refit')
        operations.append(operation)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10),
                operations=operations)


if __name__ == '__main__':
    capture('sweep1_global_shapes', request(), [
        'generate_sweep1_global_shapes.py', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleSweepGlobal20261027', timeout=240)
