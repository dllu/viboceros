"""Former mesh cutters still cut through portions removed from the document."""
from copy import deepcopy


def cases(xy):
    cutter = dict(kind='mesh', vertices=[[2., -1., -.3], [2., 4., -.3],
                                        [2., 4., .3], [2., -1., .3]], faces=[[0, 1, 2, 3]])
    target = dict(kind='mesh', vertices=[[0., 1., -.2], [4., 1., -.2],
                                        [4., 1., .2], [0., 1., .2]], faces=[[0, 1, 2, 3]])
    result = {}
    for name, picks, targets in [
            ('replaced_cutter', [[1., .5, 0.], [1., 1., .1]], [0, 2]),
            ('deleted_cutter', [[1., .5, 0.], [3., .5, 0.], [1., 1., .1]], [0, 0, 2])]:
        identifier = 'mesh_trim_cutter_policy_' + name
        result[identifier] = dict(operation='split', preselect=False,
                                  sources=deepcopy([xy, cutter, target]), first=[0, 1], second=[],
                                  trim_picks=picks, trim_targets=targets, fit_sources=True,
                                  camera=dict(target=[2., 1., 0.], location=[-8., -10., 10.]),
                                  capture_id=identifier, capture_split_topology=True, mesh_channels=True)
    return result
