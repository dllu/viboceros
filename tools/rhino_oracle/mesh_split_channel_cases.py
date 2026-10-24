"""Raw vertex seams and attribute interpolation through public MeshSplit."""
from copy import deepcopy


def cases(box, xy, wall, moved):
    result = {}
    uv = [[-1., 0.], [2., 0.], [4., 3.], [-1., 1.]]
    normals = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [.6, .8, 0.]]
    colors = [[255, 0, 10, 31], [0, 255, 20, 127], [0, 0, 255, 200], [90, 80, 70, 254]]
    decorated = dict(deepcopy(xy), texture_coordinates=uv, normals=normals, colors=colors)

    def add(name, target, cutters=None, arguments='', **extra):
        sources = [deepcopy(target)] + deepcopy([wall] if cutters is None else cutters)
        value = dict(operation='split', preselect=False, sources=sources, first=[0],
                     second=list(range(1, len(sources))), capture_split_topology=True,
                     mesh_channels=True)
        if arguments:
            value['arguments'] = arguments
        value.update(extra)
        result['mesh_split_channels_' + name] = value

    add('quad_all_post', decorated)
    add('quad_all_pre', decorated, preselect=True)
    triangle = dict(deepcopy(decorated), faces=[[0, 1, 2]])
    triangle['vertices'] = triangle['vertices'][:3]
    for key in ['texture_coordinates', 'normals', 'colors']:
        triangle[key] = triangle[key][:3]
    add('triangle_all', triangle)
    for name, field, values in [('uv', 'texture_coordinates', uv),
                                ('normals', 'normals', normals), ('colors', 'colors', colors)]:
        add('quad_' + name, dict(deepcopy(xy), **{field: deepcopy(values)}))
    add('quad_implicit', xy)
    seam = dict(kind='mesh',
                vertices=[[0., 0., 0.], [2., 0., 0.], [2., 3., 0.], [0., 3., 0.],
                          [2., 0., 0.], [4., 0., 0.], [4., 3., 0.], [2., 3., 0.]],
                faces=[[0, 1, 2, 3], [4, 5, 6, 7]],
                texture_coordinates=[[0., 0.], [1., 0.], [1., 1.], [0., 1.],
                                     [10., 0.], [11., 0.], [11., 1.], [10., 1.]],
                normals=[[.6, 0., .8]] * 4 + [[0., .8, .6]] * 4,
                colors=[[200, 10, 20, 30]] * 4 + [[10, 200, 30, 240]] * 4)
    cross = dict(kind='mesh', vertices=[[-1., 1.5, -1.], [5., 1.5, -1.],
                                        [5., 1.5, 4.], [-1., 1.5, 4.]], faces=[[0, 1, 2, 3]])
    add('seam_coplanar_post', seam, [cross])
    add('seam_coplanar_pre', seam, [cross], preselect=True)
    folded = deepcopy(seam)
    folded['vertices'][5], folded['vertices'][6] = [2., 0., 2.], [2., 3., 2.]
    add('seam_folded', folded, [cross])
    closed = deepcopy(box)
    closed.update(texture_coordinates=[[p[0], p[1] + p[2]] for p in box['vertices']],
                  normals=[[0., 0., 1.]] * len(box['vertices']),
                  colors=[[30 * i, 10 * i, 5 * i, 31 * i] for i in range(len(box['vertices']))])
    add('welded_box', closed)
    add('disjoint_discard', decorated, [moved(wall, [8., 0., 0.])])
    add('disjoint_keep', decorated, [moved(wall, [8., 0., 0.])], '_DiscardUnsplitMeshes=_No')
    partial = dict(kind='mesh', vertices=[[2., 1., -1.], [2., 4., -1.],
                                         [2., 4., 1.], [2., 1., 1.]], faces=[[0, 1, 2, 3]])
    add('partial_keep', decorated, [partial], '_DiscardUnsplitMeshes=_No')
    ngon = dict(deepcopy(decorated), ngons=[dict(vertices=[0, 1, 2, 3], faces=[0])])
    add('existing_ngon', ngon, arguments='_CreateNgons=_No _DiscardUnsplitMeshes=_Yes')
    add('infinite_plane', moved(decorated, [-2., 0., 0.]), [],
        second_arguments='_IP _WorldYZ', infinite_plane='WorldYZ')
    add('projected_curve', decorated,
        [dict(kind='polyline', points=[[2., -1., 0.], [2., 4., 0.]])])
    return result
