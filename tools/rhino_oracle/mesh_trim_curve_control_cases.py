"""Separate curve extrusion, earlier targets, face records, and cutter order."""
from copy import deepcopy


def cases(xy, wall):
    curve = dict(kind='polyline', points=[[2., -1., 0.], [2., 4., 0.]])
    target = dict(kind='mesh', vertices=[[0., 1., -1.], [4., 1., -1.],
                                        [4., 1., 1.], [0., 1., 1.]], faces=[[0, 1, 2, 3]])
    result = {}
    for name, mesh_cutter, first, after, triangles in [
            ('curve_after_quad', False, [0, 1], True, False),
            ('curve_direct_quad', False, [0, 1], False, False),
            ('curve_only_quad', False, [1], False, False),
            ('mesh_after_quad', True, [0, 1], True, False),
            ('mesh_only_quad', True, [1], False, False),
            ('horizontal_only_quad', False, [0], False, False),
            ('curve_after_triangles', False, [0, 1], True, True),
            ('curve_direct_triangles', False, [0, 1], False, True),
            ('curve_after_reverse', False, [1, 0], True, False)]:
        identifier = 'mesh_trim_curve_controls_' + name
        sources = deepcopy([xy, wall if mesh_cutter else curve, target])
        if triangles:
            sources[2]['faces'] = [[0, 1, 2], [0, 2, 3]]
        result[identifier] = dict(operation='split', preselect=False, sources=sources,
                                  first=first, second=[], capture_id=identifier,
                                  trim_picks=([[1., .5, 0.]] if after else []) + [[1., 1., .5]],
                                  trim_targets=([0] if after else []) + [2], fit_sources=True,
                                  camera=dict(target=[2., 1., 0.], location=[-8., -10., 10.]),
                                  cplane=[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
                                  capture_split_topology=True, mesh_channels=True)
    return result
