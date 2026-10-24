"""Does the removed part of a former cutter still cut subsequent targets?"""
from copy import deepcopy


def cases(xy):
    curve = dict(kind='polyline', points=[[2., -1., 0.], [2., 4., 0.]])
    target = dict(kind='mesh', vertices=[[0., 1., -1.], [4., 1., -1.],
                                        [4., 1., 1.], [0., 1., 1.]], faces=[[0, 1, 2, 3]])
    result = {}
    for name, picks, targets in [
            ('replaced_cutter', [[1., .5, 0.], [1., 1., .5]], [0, 2]),
            ('deleted_cutter', [[1., .5, 0.], [3., .5, 0.], [1., 1., .5]], [0, 0, 2])]:
        identifier = 'mesh_trim_snapshot_policy_' + name
        result[identifier] = dict(operation='split', preselect=False,
                                  sources=deepcopy([xy, curve, target]), first=[0, 1], second=[],
                                  trim_picks=picks, trim_targets=targets, fit_sources=True,
                                  camera=dict(target=[2., 1., 0.], location=[-8., -10., 10.]),
                                  capture_id=identifier, capture_split_topology=True, mesh_channels=True)
    return result
