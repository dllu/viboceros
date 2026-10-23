"""Independent exhaustive raw-index and cyclic-origin seed controls."""
from itertools import permutations, product
from copy import deepcopy


def sdk_cases():
    points = [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.]]
    result = {}
    for indices in permutations(range(4)):
        vertices = [None] * 4
        for old, new in enumerate(indices):
            vertices[new] = points[old][:]
        for a, b, order in product(range(3), range(3), range(2)):
            faces = []
            for source, offset in zip([[0, 1, 2], [0, 3, 2]], [a, b]):
                cycle = source[offset:] + source[:offset]
                faces.append([indices[v] for v in cycle])
            if order:
                faces.reverse()
            name = 'unify_seed_' + ''.join(map(str, indices)) + '_' + str(a) + str(b) + str(order)
            result[name] = dict(vertices=[p[:] for p in vertices], faces=faces)
    return result


def followup_cases():
    square = [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.]]
    cases = {}

    def add(name, points, faces, normals=False):
        case = dict(vertices=deepcopy(points), faces=deepcopy(faces))
        if normals:
            case['normals'] = [[1., 0., 0.] for _ in points]
        cases['unify_followup_' + name] = case

    def rotate(face, count):
        return face[count:] + face[:count]

    for mapping in [(0, 1, 2, 3, 4, 5), (5, 4, 3, 2, 1, 0),
                    (1, 2, 3, 4, 5, 0), (3, 1, 5, 0, 4, 2)]:
        points = [None] * 6
        for old, new in enumerate(mapping):
            points[new] = (square[:3] + [square[0], square[3], square[2]])[old]
        for a, b in product(range(3), repeat=2):
            faces = [[mapping[v] for v in rotate(f, r)]
                     for f, r in zip([[0, 1, 2], [3, 4, 5]], [a, b])]
            add('seam_' + ''.join(map(str, mapping)) + '_' + str(a) + str(b), points, faces, True)

    grid = [[0., 0., 0.], [1., 0., 0.], [2., 0., 0.],
            [0., 1., 0.], [1., 1., 0.], [2., 1., 0.]]
    for mapping in [(0, 1, 2, 3, 4, 5), (5, 4, 3, 2, 1, 0)]:
        points = [None] * 6
        for old, new in enumerate(mapping):
            points[new] = grid[old]
        for a, b, order in product(range(4), range(4), range(2)):
            faces = [[mapping[v] for v in rotate(f, r)]
                     for f, r in zip([[0, 1, 4, 3], [4, 5, 2, 1]], [a, b])]
            if order:
                faces.reverse()
            add('quad_' + ''.join(map(str, mapping)) + '_' + str(a) + str(b) + str(order), points, faces)

    for active, order in product(range(4), range(2)):
        points, faces = [], []
        for region in range(3):
            points.extend([[x + 5 * region, y, z] for x, y, z in square])
            for face in [[0, 1, 2], [0, 3, 2]]:
                cycle = face if active == region else rotate(face, 1)
                faces.append([v + 4 * region for v in cycle])
        if order:
            faces.reverse()
        add('regions_' + str(active) + str(order), points, faces)

    for prefix, point in enumerate([square[0], square[2], [99., 99., 99.]]):
        for a, b in product(range(3), repeat=2):
            faces = [[v + 1 for v in rotate(f, r)]
                     for f, r in zip([[0, 1, 2], [0, 3, 2]], [a, b])]
            add('unused_alias_' + str(prefix) + str(a) + str(b), [point] + square, faces, True)

    def scene(source, **extra):
        result = dict(command='UnifyMeshNormals', whole=True, preselected=True,
                      sources=[dict(deepcopy(source), kind='mesh')], mesh_channels=True)
        result.update(extra)
        return result

    for reverse, rotation, member_order in product(range(2), range(4), range(2)):
        cycle = [0, 3, 2, 1] if reverse else [0, 1, 2, 3]
        source = dict(vertices=square + [[3., .5, 1.]],
                      faces=[[1, 2, 4], [0, 1, 2], [0, 2, 3]],
                      normals=[[1., 0., 0.]] * 5,
                      ngons=[dict(vertices=rotate(cycle, rotation),
                                  faces=[2, 1] if member_order else [1, 2])])
        cases['unify_followup_command_ngon_' + str(reverse) + str(rotation) + str(member_order)] = scene(source)
    points = [square[1], square[0], square[3], square[2]]
    cases['unify_followup_command_reindex'] = scene(dict(vertices=points, faces=[[1, 0, 3], [1, 2, 3]]))
    cases['unify_followup_command_ngon_noop'] = scene(dict(vertices=square,
        faces=[[0, 1, 2], [0, 2, 3]], ngons=[dict(vertices=[2, 1, 0, 3], faces=[1, 0])]))
    shifted = cases['unify_followup_command_ngon_010']['sources'][0]
    cases['unify_followup_command_ngon_postselect'] = scene(shifted, whole=False,
        preselected=False, command_arguments='_All _Enter')
    mixed = scene(shifted)
    mixed['sources'].append(dict(kind='mesh', vertices=[[x + 10, y, z] for x, y, z in square],
                                faces=[[0, 1, 2], [0, 2, 3]]))
    cases['unify_followup_command_ngon_peer'] = mixed
    return cases
