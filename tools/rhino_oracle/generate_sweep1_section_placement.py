"""Public Sweep1 placement with interior, offset and reversed input profiles."""
import copy
from .capture_family import capture


def line(a, b):
    return dict(type='line', start=a, end=b)


def request():
    rail = line([0., 0., 0.], [0., 0., 5.])
    profiles = {
        'start': [line([0., 0., 0.], [2., 0., 0.])],
        'interior': [line([0., 0., 2.], [2., 0., 2.])],
        'end': [line([0., 0., 5.], [2., 0., 5.])],
        'offset': [line([2., 0., 2.], [4., 0., 2.])],
        'sloped': [line([1., 0., 1.], [3., 0., 4.])],
        'two-interior': [line([0., 0., 1.], [2., 0., 1.]),
                         line([0., 0., 4.], [3., 0., 4.])],
        'two-reversed': [line([0., 0., 4.], [3., 0., 4.]),
                         line([0., 0., 1.], [2., 0., 1.])],
    }
    parameters = {'start': [0.], 'interior': [2.], 'end': [5.],
                  'offset': [2.], 'sloped': [1.],
                  'two-interior': [1., 4.], 'two-reversed': [4., 1.]}
    operations = []
    for name, sections in profiles.items():
        for command in [False, True]:
            operations.append(dict(
                op='sweep1', id='sweep1-placement-{}-{}'.format(name, 'command' if command else 'sdk'),
                rail=copy.deepcopy(rail), sections=copy.deepcopy(sections),
                parameters=parameters[name], command=command, refit_rail=False,
                queries=[[x, 0., z] for z in [0., 1., 2., 3., 4., 5.]
                         for x in [0., 1., 2., 3., 4.]]))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10),
                operations=operations)


if __name__ == '__main__':
    capture('sweep1_section_placement', request(), [
        'generate_sweep1_section_placement.py', 'rhino_worker.py', 'client.py',
        'structural_sdk.py'], 'VibocerosOracleSweep1Placement20261027')
