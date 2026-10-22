"""Fixed chain-option and existing-anchor observations for SubDCrease."""
from copy import deepcopy


def cases(cube, label):
    chain = [[0, 1], [1, 2], [2, 3]]
    result = {}

    def selected(edges, **options):
        definition = cube(**options)
        definition['edges'] = deepcopy(edges)
        return definition

    def add(name, sources, actions):
        result['chains_' + name] = dict(sources=sources, actions=actions,
                                       live=True, fit_sources=True)

    for weight in [50, 24.75, 25.25]:
        for where, vertex in [('end', 0), ('middle', 1)]:
            add('seed_' + str(weight).replace('.', '_') + '_' + where,
                [selected(chain, soft=weight)], [label(vertex), 80, 'Enter'])
    for where, vertex in [('end', 0), ('middle', 1)]:
        add('hard_' + where, [selected(chain, creases=deepcopy(chain))],
            [label(vertex), 80, 'Enter'])
        add('partial_soft_' + where,
            [selected(chain, soft=50, seed_edges=[[0, 1]])],
            [label(vertex), 80, 'Enter'])
        add('partial_hard_' + where, [selected(chain, creases=[[0, 1]])],
            [label(vertex), 80, 'Enter'])

    for name, edges, vertex in [('branch', [[0, 1], [0, 3], [0, 4]], 1),
                                ('disconnected', [[0, 1], [6, 7]], 0)]:
        for option, actions in [('hard', ['CreaseThisChain', 'Enter']),
                                ('constant', ['ConstantWeightChain', 70, 'Enter']),
                                ('hard_cancel', ['CreaseThisChain', 'Cancel']),
                                ('constant_cancel', ['ConstantWeightChain', 'Cancel'])]:
            add(name + '_' + option, [selected(edges)], [label(vertex)] + actions)
    other = cube(soft=50)
    other['vertices'] = [[x + 9, y - 4, z + 3] for x, y, z in other['vertices']]
    for option, actions in [('hard', ['CreaseThisChain', 'Enter']),
                            ('constant', ['ConstantWeightChain', 70, 'Enter'])]:
        add('multiple_' + option, [selected(chain), deepcopy(other)], [label(0)] + actions)

    for weight in [0, 50]:
        for vertex in [0, 1]:
            definition = selected([[0, 1], [1, 2], [0, 2]], soft=weight)
            definition['vertices'] = [[0, 0, 0], [4, 0, 0], [0, 3, 0],
                                      [0, 0, 2], [4, 0, 2], [0, 3, 2]]
            definition['faces'] = [[0, 2, 1], [3, 4, 5], [0, 1, 4, 3],
                                   [1, 2, 5, 4], [2, 0, 3, 5]]
            add('odd_loop_' + str(weight) + '_vertex_' + str(vertex),
                [definition], [label(vertex), 80, 'Enter'])
    return result
