"""Trim-side picks, unchanged targets, temporary planes, and accepted steps."""
from copy import deepcopy


def cases(box, xy, wall, moved, channels):
    result = {}
    def add(name, sources, picks, **extra):
        identifier = 'mesh_trim_controls_' + name
        value = dict(operation='split', preselect=False, sources=deepcopy(sources),
                     first=list(range(1, len(sources))), second=[], trim_picks=deepcopy(picks),
                     capture_id=identifier, capture_split_topology=True, mesh_channels=True,
                     sequence=True)
        value.update(extra)
        result[identifier] = value
    add('sheet_left_post', [xy, wall], [[1., 1., 0.]])
    add('sheet_left_pre', [xy, wall], [[1., 1., 0.]], preselect=True)
    add('box_left_pre', [box, wall], [[1., 1., 2.]], preselect=True)
    add('disjoint', [xy, moved(wall, [8., 0., 0.])], [[1., 1., 0.]])
    partial = dict(kind='mesh', vertices=[[2., 1., -1.], [2., 4., -1.],
                                         [2., 4., 1.], [2., 1., 1.]], faces=[[0, 1, 2, 3]])
    add('partial', [xy, partial], [[1., .5, 0.]])
    add('closed_cutter', [box, moved(box, [2., 1., 1.])], [[1., .5, 2.]])
    add('curve', [xy, dict(kind='polyline', points=[[2., -1., 0.], [2., 4., 0.]])], [[1., 1., 0.]])
    add('plane', [xy], [[1., 1., 0.]], cutting_arguments='_IP _3Point 2,0,0 2,1,0 2,0,1',
        infinite_plane='2,0,0,1,0,0')
    cross = dict(kind='mesh', vertices=[[-1., 1.5, -1.], [5., 1.5, -1.],
                                       [5., 1.5, 3.], [-1., 1.5, 3.]], faces=[[0, 1, 2, 3]])
    add('two_cutters', [box, wall, cross], [[1., .5, 2.]])
    add('channels', [channels['mesh_split_channels_quad_all_post']['sources'][0], wall], [[1., 1., 0.]])
    add('cancel_after', [xy, wall], [[1., 1., 0.]], finish='Cancel')
    add('both_sides', [xy, wall], [[1., 1., 0.], [3., 1., 0.]])
    add('shared_cutters', [xy, wall], [[1., 1., 0.]], first=[0, 1])
    return result
