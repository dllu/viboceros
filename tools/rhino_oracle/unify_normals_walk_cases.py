"""Mixed connected regions distinguish global and per-region orientation seeds."""
from copy import deepcopy
from itertools import product
import math


def source(states, phase, reverse_order):
    points, faces, ngons = [], [], []
    square = [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.]]
    for region, state in enumerate(states):
        points.extend([[x + 5 * region, y, z] for x, y, z in square])
        first_face = len(faces)
        for side, triangle in enumerate([[0, 1, 2], [0, 2, 3]]):
            negative = state in ([2, 3] if side == 0 else [1, 3])
            if negative:
                triangle = list(reversed(triangle))
            faces.append([v + 4 * region for v in triangle])
        if state in [0, 3]:
            cycle = [4 * region + v for v in [1, 2, 3, 0]]
            ngons.append(dict(vertices=cycle, faces=[first_face + 1, first_face]))
    if reverse_order:
        faces.reverse()
        for ngon in ngons:
            ngon['faces'] = [len(faces) - 1 - f for f in ngon['faces']]
    faces[0] = faces[0][phase:] + faces[0][:phase]
    return dict(vertices=points, faces=faces, ngons=ngons,
                normals=[[1., 0., 0.] for _ in points],
                uv=[[i * .25, i * .5] for i in range(len(points))],
                colors=[[(17 * i) % 256, (31 * i) % 256, (47 * i) % 256, (13 * i) % 256]
                        for i in range(len(points))])


def command(case, **extra):
    mesh = deepcopy(case)
    mesh['kind'] = 'mesh'
    if 'uv' in mesh:
        mesh['texture_coordinates'] = mesh.pop('uv')
    if 'colors' in mesh:
        mesh['colors'] = [c[:3] + [255 - c[3]] for c in mesh['colors']]
    value = dict(command='UnifyMeshNormals', whole=True, preselected=True,
                 sources=[mesh], mesh_channels=True)
    value.update(extra)
    return value


def walk_cases():
    cases = {}
    for states, phase, order in product([[0, 1], [3, 1], [1, 0], [1, 3], [1, 1], [3, 3]],
                                       [0, 1], range(2)):
        name = 'unify_walk_' + ''.join(map(str, states)) + '_' + str(phase) + str(order)
        cases[name] = source(states, phase, order)
    for states in [[0, 1], [3, 1], [1, 0], [1, 3], [1, 1], [3, 3]]:
        name = 'unify_walk_command_' + ''.join(map(str, states))
        cases[name] = command(source(states, 1, False))
    return cases


def component_cases(previous_followups):
    cases = {}
    for regions in [2, 3]:
        for states, phase, order in product(product(range(4), repeat=regions), range(3), range(2)):
            name = 'unify_component_pairs_' + ''.join(map(str, states)) + '_' + str(phase) + str(order)
            cases[name] = source(states, phase, order)

    def add(name, points, faces, phase, order):
        faces = deepcopy(faces)
        if order:
            faces.reverse()
        faces[0] = faces[0][phase:] + faces[0][:phase]
        cases['unify_component_' + name] = dict(vertices=deepcopy(points), faces=faces,
            normals=[[1., 0., 0.] for _ in points],
            uv=[[i * .25, i * .5] for i in range(len(points))],
            colors=[[(17 * i) % 256, (31 * i) % 256, (47 * i) % 256, (13 * i) % 256]
                    for i in range(len(points))])

    square = [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.]]
    for phase, order in product(range(3), range(2)):
        suffix = str(phase) + str(order)
        add('isolated_' + suffix, square + [[10., 0., 1.], [11., 0., 1.], [10., 1., 1.]],
            [[0, 1, 2], [0, 3, 2], [4, 5, 6]], phase, order)
        add('contact_' + suffix, square + [[0., 0., 2.], [0., 1., 2.]],
            [[0, 1, 2], [0, 3, 2], [0, 4, 5]], phase, order)
        add('nonmanifold_' + suffix, square + [[1., .5, 1.], [10., 0., 0.], [11., 0., 0.], [10., 1., 0.]],
            [[0, 1, 2], [0, 3, 2], [0, 4, 2], [5, 6, 7]], phase, order)

    tetra = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
    tetra_faces = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]
    for mode, phase, order in product(range(3), range(3), range(2)):
        faces = deepcopy(tetra_faces)
        if mode == 1:
            faces = [list(reversed(f)) for f in faces]
        elif mode == 2:
            faces[0].reverse()
        add('tetra_' + str(mode) + str(phase) + str(order),
            tetra + [[x + 5, y, z] for x, y, z in square], faces + [[4, 5, 6], [4, 7, 6]], phase, order)

    points, faces = [], []
    for i in range(4):
        angle = 2. * math.pi * i / 4.
        for lateral in [-.25, .25]:
            radius = 2. + lateral * math.cos(angle * .5)
            points.append([radius * math.cos(angle), radius * math.sin(angle), lateral * math.sin(angle * .5)])
    for i in range(4):
        a = 2 * i
        na, nb = (1, 0) if i == 3 else (a + 2, a + 3)
        faces.extend([[a, na, a + 1], [na, nb, a + 1]])
    for phase, order, append in product(range(3), range(2), range(2)):
        add('mobius_' + str(phase) + str(order) + str(append),
            points + ([[x + 10, y, z] for x, y, z in square] if append else []),
            faces + ([[8, 9, 10], [8, 11, 10]] if append else []), phase, order)

    for name, case in previous_followups.items():
        if name.startswith('unify_followup_regions_'):
            cases[name.replace('unify_followup_regions_', 'unify_component_command_regions_')] = command(case)
    for name in ['isolated_10', 'tetra_110', 'contact_10', 'mobius_100']:
        cases['unify_component_command_' + name] = command(cases['unify_component_' + name])
    return cases
