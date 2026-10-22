"""Repeated edits target positions measured from prior native observations."""
from copy import deepcopy
import json
import os


def cases(cube, label):
    with open(os.path.join(os.path.dirname(__file__), 'fixtures',
                           'soft_crease_followup_assets.json')) as stream:
        assets = json.load(stream)
    result = {}
    chain = [[0, 1], [1, 2], [2, 3]]

    def source(edges, **options):
        definition = cube(**options)
        definition['edges'] = deepcopy(edges)
        return definition

    def pick(vertex, key):
        action = label(vertex)
        action.update(deepcopy(assets['points'][key]))
        return action

    def add(name, definition, actions):
        result['followup_' + name] = dict(sources=[definition], actions=actions,
                                         live=True, fit_sources=True)

    for name, first, second, key in [('start_middle', 0, 1, 'start_to_middle'),
                                    ('middle_start', 1, 0, 'middle_to_start')]:
        add(name, source(chain), [label(first), 80, pick(second, key), 20, 'Enter'])
    add('start_middle_cancel', source(chain),
        [label(0), 80, pick(1, 'start_to_middle'), 20, 'Cancel'])
    for vertex in [0, 1]:
        add('constant_then_vertex_' + str(vertex), source(chain),
            [50, pick(vertex, 'constant50_to_' + str(vertex)), 20, 'Enter'])
    add('weighted_repeated', source(chain, soft=50),
        [label(0), 80, pick(1, 'seed50_to_middle'), 20, 'Enter'])
    add('even_repeated', source([[0, 1], [1, 2], [2, 3], [0, 3]]),
        [label(0), 80, pick(1, 'even_to_middle'), 20, 'Enter'])
    odd = source([[0, 1], [1, 2], [0, 2]])
    odd['vertices'] = [[0, 0, 0], [4, 0, 0], [0, 3, 0],
                       [0, 0, 2], [4, 0, 2], [0, 3, 2]]
    odd['faces'] = [[0, 2, 1], [3, 4, 5], [0, 1, 4, 3], [1, 2, 5, 4], [2, 0, 3, 5]]
    add('odd_repeated', odd, [label(0), 80, pick(1, 'odd_to_middle'), 20, 'Enter'])

    for row in ['label_start_accepted', 'label_middle_accepted']:
        for vertex in [0, 2]:
            definition = source(chain, native_archive=deepcopy(assets['archives'][row]))
            add('archive_' + row + '_vertex_' + str(vertex), definition,
                [label(vertex), 20, 'Enter'])
    for weight in [0, 50]:
        definition = source([[0, 1], [1, 2], [2, 3], [3, 4], [0, 4]], soft=weight)
        bottom = [[0, 0, 0], [4, 0, 0], [5, 3, 0], [2, 5, 0], [-1, 3, 0]]
        definition['vertices'] = bottom + [[x, y, 2] for x, y, _ in bottom]
        definition['faces'] = [[0, 3, 2, 1], [0, 4, 3], [5, 6, 7, 8], [5, 8, 9]]
        definition['faces'] += [[i, (i + 1) % 5, (i + 1) % 5 + 5, i + 5] for i in range(5)]
        add('pentagon_' + str(weight), definition, [label(0), 80, 'Enter'])
    long_chain = chain + [[3, 7]]
    add('distant_hard', source(long_chain, creases=[[0, 1], [1, 2]]), [label(0), 80, 'Enter'])
    add('distant_soft', source(long_chain, soft=50, seed_edges=[[1, 2]]), [label(0), 80, 'Enter'])
    for weight in [-1, 101]:
        add('invalid_' + str(weight).replace('-', 'minus'), source(chain),
            [label(0), weight, 80, 'Enter'])
    return result


def anchor_cases(cube, label):
    result = {}
    chain = [[0, 1], [1, 2], [2, 3]]

    def add(name, definition, actions):
        result['anchors_' + name] = dict(sources=[definition], actions=actions,
                                        live=True, fit_sources=True)
    for weight in [0, 50]:
        definition = cube(soft=weight)
        definition['edges'] = deepcopy(chain)
        add('noop_middle_' + str(weight), definition,
            [label(1), weight, label(0), 80, 'Enter'])
    definition = cube()
    definition['edges'] = [[0, 1], [1, 2], [2, 3], [0, 3]]
    add('loop_noop_middle', definition, [label(1), 0, label(0), 80, 'Enter'])
    definition = cube(soft=50, seed_edges=[[0, 1]])
    definition['edges'] = deepcopy(chain)
    add('erase_break', definition, [label(1), 0, label(0), 80, 'Enter'])
    return result
