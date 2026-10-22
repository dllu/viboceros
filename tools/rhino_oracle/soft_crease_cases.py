"""Fixed native soft-crease discovery recipes; no arbitrary macro input."""
from copy import deepcopy
if __package__:
    from .subdivide_probe import source, CUBE, CUBE_FACES, STRIP
else:
    from subdivide_probe import source, CUBE, CUBE_FACES, STRIP


def cube(**options):
    return source(vertices=deepcopy(CUBE), faces=deepcopy(CUBE_FACES), edges=[[0, 1]], **options)


DISCOVERY = {}
for weight in [0, 1, 10, 24.9, 25, 25.1, 50, 75, 90, 99, 100]:
    DISCOVERY['soft_constant_' + str(weight).replace('.', '_')] = dict(sources=[cube()], actions=[weight, 'Enter'])
DISCOVERY.update({
    'soft_default': dict(sources=[cube()], actions=['Enter']),
    'soft_cancel_empty': dict(sources=[cube()], actions=['Cancel']),
    'soft_cancel_after_weight': dict(sources=[cube()], actions=[50, 'Cancel']),
    'soft_constant_option': dict(sources=[cube()], actions=['ConstantWeight', 50, 'Enter']),
    'soft_two_weights': dict(sources=[cube()], actions=[20, 80, 'Enter']),
    'soft_crease_all': dict(sources=[cube()], actions=['CreaseAll', 'Enter']),
    'soft_text_sizes': dict(sources=[cube()], actions=['SmallerText', 'LargerText', 50, 'Enter']),
    'soft_hard_source': dict(sources=[cube(creases=[[0, 1]])], actions=[50, 'Enter']),
    'soft_corner_source': dict(sources=[cube(corners=[0])], actions=[50, 'Enter']),
    'soft_boundary': dict(sources=[source(edges=[[0, 1]])], actions=[50, 'Enter']),
    'soft_open_interior': dict(sources=[source(vertices=STRIP, faces=[[0, 1, 4, 3], [1, 2, 5, 4]], edges=[[1, 4]])], actions=[50, 'Enter']),
    'soft_whole_object': dict(sources=[cube()], whole=True, actions=[50, 'Enter']),
    'soft_vertex_input': dict(sources=[cube()], vertex_only=True, actions=[50, 'Enter']),
    'soft_start_label': dict(sources=[cube()], actions=[dict(vertex=0, space='surface'), 10, 'Enter']),
    'soft_start_control_label': dict(sources=[cube()], actions=[dict(vertex=0, space='control'), 10, 'Enter']),
})
for name, edges in [('chain', [[0, 1], [1, 2], [2, 3]]), ('branch', [[0, 1], [0, 3], [0, 4]]), ('loop', [[0, 1], [1, 2], [2, 3], [0, 3]])]:
    definition = cube()
    definition['edges'] = edges
    DISCOVERY['soft_' + name] = dict(sources=[definition], actions=[50, 'Enter'])
    DISCOVERY['soft_' + name + '_label'] = dict(sources=[definition], actions=[dict(vertex=0, space='surface'), 10, 'Enter'])

SCENES = {}


def all_cases():
    return dict(DISCOVERY, **SCENES)
