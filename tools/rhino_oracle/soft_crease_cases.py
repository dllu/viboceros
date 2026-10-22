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
for weight in [0, .01, .1, .24, .25, .26, 1, 10, 24.74, 24.75, 24.9, 25,
               25.1, 25.25, 25.26, 33.333, 50, 75, 99.75, 99.9, 100]:
    SCENES['weight_' + str(weight).replace('.', '_')] = dict(sources=[cube()], actions=[weight, 'Enter'])
for name, options in [('hard', dict(creases=[[0, 1]])), ('corner', dict(corners=[0])),
                      ('soft', dict(soft=50)), ('side_soft', dict(soft=50, seed_edges=[[0, 3]]))]:
    for suffix, actions in [('zero', [0, 'Enter']), ('half', [50, 'Enter']),
                            ('maximum', [100, 'Enter']), ('hard', ['CreaseAll', 'Enter']),
                            ('default', ['Enter']), ('cancel', [75, 'Cancel'])]:
        SCENES[name + '_' + suffix] = dict(sources=[cube(**options)], actions=actions)
for name, edges in [('chain', [[0, 1], [1, 2], [2, 3]]),
                    ('branch', [[0, 1], [0, 3], [0, 4]]),
                    ('loop', [[0, 1], [1, 2], [2, 3], [0, 3]]),
                    ('disconnected', [[0, 1], [6, 7]])]:
    for suffix, actions in [('fraction', [33.333, 'Enter']), ('hard', ['CreaseAll', 'Enter'])]:
        definition = cube()
        definition['edges'] = edges
        SCENES[name + '_' + suffix] = dict(sources=[definition], actions=actions)
for name, actions in [('constant_empty', ['ConstantWeight', 'Enter', 'Enter']),
                      ('constant_crease', ['ConstantWeight', 'Crease', 'Enter']),
                      ('constant_cancel', ['ConstantWeight', 'Cancel']),
                      ('constant_weight_cancel', ['ConstantWeight', 75, 'Cancel']),
                      ('constant_repeat', ['ConstantWeight', 20, 'ConstantWeight', 80, 'Enter']),
                      ('return_smooth', [50, 0, 'Enter']), ('hard_then_soft', ['CreaseAll', 50, 'Enter']),
                      ('soft_then_hard', [50, 'CreaseAll', 'Enter']),
                      ('text_only', ['SmallerText', 'LargerText', 'Enter'])]:
    SCENES[name] = dict(sources=[cube()], actions=actions)
for weight in [0, 50, 100]:
    strip = source(vertices=STRIP, faces=[[0, 1, 4, 3], [1, 2, 5, 4]], edges=[[0, 1], [1, 4]])
    SCENES['strip_' + str(weight)] = dict(sources=[strip], actions=[weight, 'Enter'])
for name, actions in [('edit', [75, 'Enter']), ('cancel', [75, 'Cancel']), ('noop', ['Enter'])]:
    definition = cube()
    definition['selected_grips'] = [0, 3]
    SCENES['grips_' + name] = dict(sources=[definition], actions=actions, grips=True)
for name, actions in [('weight', [33.333, 'Enter']), ('hard', ['CreaseAll', 'Enter']),
                      ('cancel', [50, 'Cancel']), ('noop', ['Enter'])]:
    other = cube(soft=50)
    other['vertices'] = [[x + 9, y - 4, z + 3] for x, y, z in other['vertices']]
    SCENES['multiple_' + name] = dict(sources=[cube(), other], actions=actions)

# The captured main producer reused the discovery name for a seeded source.
# Its immutable source version disambiguates that historical row. New requests
# keep a globally unique recipe name for the seeded source.
SCENES['seed_soft_default'] = SCENES.pop('soft_default')


LABELS = {}
for name, vertex, space, grips in [('start', 0, 'surface', False),
                                   ('middle', 1, 'surface', False),
                                   ('control', 0, 'control', True)]:
    definition = cube()
    definition['edges'] = [[0, 1], [1, 2], [2, 3]]
    LABELS['label_' + name] = dict(sources=[definition], live=True, grips=grips,
        actions=[dict(vertex=vertex, space=space), 80, 'Enter', 'Enter'])

EXTRA = {}
for i, weight in enumerate([.24999999999999997, .25, .25000000000000006,
        24.749999999999996, 24.75, 24.750000000000004,
        25.249999999999996, 25.25, 25.250000000000004,
        49.74999999999999, 49.75, 49.75000000000001,
        50.24999999999999, 50.25, 50.25000000000001,
        74.75, 75.25, 99.75]):
    EXTRA['boundary_' + str(i)] = dict(sources=[cube()], actions=[weight, 'Enter'], roundtrip=True)
for name, case in LABELS.items():
    EXTRA[name + '_input'] = deepcopy(case)
    EXTRA[name + '_input']['fit_sources'] = True

PRECISION = {name + '_precise': deepcopy(case) for name, case in EXTRA.items()}
NUMERIC_LABELS = {name + '_numeric': deepcopy(case) for name, case in EXTRA.items() if name.startswith('boundary_')}
for name, case in LABELS.items():
    recipe = deepcopy(case)
    recipe['actions'] = recipe['actions'][:-1]
    recipe['fit_sources'] = True
    if name == 'label_control':
        recipe['actions'][0]['space'] = 'surface'
    NUMERIC_LABELS[name + '_accepted'] = recipe
def label(vertex, source=0):
    return dict(vertex=vertex, source=source, space='surface')
for name, actions in [('both_ends', [label(0), 20, label(3), 90, 'Enter']),
                      ('after_constant', [50, label(0), 20, 'Enter']),
                      ('cancel_outer', [label(0), 80, 'Cancel']),
                      ('cancel_inner', [label(0), 'Cancel'])]:
    recipe = deepcopy(NUMERIC_LABELS['label_start_accepted'])
    recipe['actions'] = actions
    NUMERIC_LABELS['label_' + name] = recipe
for name, edges in [('branch', [[0, 1], [0, 3], [0, 4]]),
                    ('loop', [[0, 1], [1, 2], [2, 3], [0, 3]]),
                    ('disconnected', [[0, 1], [6, 7]])]:
    recipe = deepcopy(NUMERIC_LABELS['label_start_accepted'])
    recipe['sources'][0]['edges'] = edges
    NUMERIC_LABELS['label_' + name] = recipe
recipe = deepcopy(NUMERIC_LABELS['label_start_accepted'])
other = cube(soft=50)
other['vertices'] = [[x + 9, y - 4, z + 3] for x, y, z in other['vertices']]
recipe['sources'].append(other)
NUMERIC_LABELS['label_multiple'] = recipe

if __package__:
    from .soft_crease_chain_cases import cases as chain_cases
    from .soft_crease_followup_cases import cases as followup_cases, anchor_cases
else:
    from soft_crease_chain_cases import cases as chain_cases
    from soft_crease_followup_cases import cases as followup_cases, anchor_cases
CHAINS = chain_cases(cube, label)
FOLLOWUP = followup_cases(cube, label)
ANCHORS = anchor_cases(cube, label)


def all_cases():
    families = (DISCOVERY, SCENES, LABELS, EXTRA, PRECISION, NUMERIC_LABELS,
                CHAINS, FOLLOWUP, ANCHORS)
    pairs = [pair for family in families for pair in family.items()]
    result = dict(pairs)
    if len(result) != len(pairs):
        raise ValueError('soft crease recipe names must be globally unique')
    return result
