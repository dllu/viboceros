"""Repeated physical picks after an original cutter has been replaced."""
from copy import deepcopy


def cases(box, xy, wall, moved):
    result = {}

    def add(name, sources, picks, **extra):
        identifier = 'mesh_trim_continuation_' + name
        value = dict(operation='split', preselect=False, sources=deepcopy(sources),
                     first=list(range(len(sources))), second=[], trim_picks=picks,
                     capture_id=identifier, capture_split_topology=True, mesh_channels=True)
        value.update(extra)
        result[identifier] = value

    add('shared_both_sides', [xy, wall], [[1., 1., 0.], [3., 1., 0.]])
    add('shared_cancel_after_two', [xy, wall], [[1., 1., 0.], [3., 1., 0.]], finish='Cancel')
    cross = dict(kind='mesh', vertices=[[-1., 1.5, -1.], [5., 1.5, -1.],
                                       [5., 1.5, 3.], [-1., 1.5, 3.]], faces=[[0, 1, 2, 3]])
    add('shared_two_cutters', [box, wall, cross], [[1., .5, 2.], [3., .5, 2.]])
    sources = [xy, wall, moved(xy, [6., 0., 0.]), moved(wall, [6., 0., 0.])]
    add('shared_other_target', sources, [[1., 1., 0.], [7., 1., 0.]],
        first=[0, 1, 3], trim_targets=[0, 2], fit_sources=True)
    long_wall = dict(kind='mesh', vertices=[[2., -1., -2.], [2., 8., -2.],
                                           [2., 8., 2.], [2., -1., 2.]], faces=[[0, 1, 2, 3]])
    add('two_targets_control', [xy, long_wall, moved(xy, [0., 4., 0.])],
        [[1., 1., 0.], [3., 5., 0.]], first=[1], trim_targets=[0, 2], fit_sources=True)
    curve = dict(kind='polyline', points=[[2., -1., 0.], [2., 8., 0.]])
    add('shared_curve_other_target', [xy, curve, moved(xy, [0., 4., 0.])],
        [[1., 1., 0.], [3., 5., 0.]], first=[0, 1], trim_targets=[0, 2], fit_sources=True)
    return result
