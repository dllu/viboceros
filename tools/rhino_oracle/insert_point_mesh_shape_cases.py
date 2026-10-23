"""Independent shape and spatial holdouts for triangle/quad pairing."""
from copy import deepcopy
from math import cos, sin, pi
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source
CASES = {}


def case(name, vertices, points, reverse=False):
    shape = source('mesh', vertices=deepcopy(vertices),
                   faces=[[0, 3, 2, 1] if reverse else [0, 1, 2, 3]])
    CASES['insert_point_shape_'+name] = dict(command='InsertPoint', sources=[shape],
        whole=True, preselected=True, live=True, mesh_channels=False, points=points,
        actions=[dict(click=p) for p in points]+['Enter'])


trapezoid = [[0., 0., 0.], [5., 0., 0.], [4., 3., 0.], [1., 3., 0.]]
case('trapezoid', trapezoid, [[.5, 1.5, 0.]])
case('trapezoid_reversed', trapezoid, [[.5, 1.5, 0.]], reverse=True)
case('trapezoid_adjacent', trapezoid, [[.5, 1.5, 0.], [2.5, 3., 0.]])
rhombus = [[0., 0., 0.], [4., 0., 0.], [5., 3., 0.], [1., 3., 0.]]
case('rhombus', rhombus, [[.5, 1.5, 0.]])
case('rhombus_same_edge', rhombus, [[.25, .75, 0.], [.75, 2.25, 0.]])
tall = [[0., 0., 0.], [2., 0., 0.], [2., 8., 0.], [0., 8., 0.]]
case('tall_low', tall, [[0., 2., 0.]])
case('tall_high', tall, [[0., 6., 0.]])
quad = [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
def rotate(p):
    x, y, z = p
    return [x*cos(pi/6)-y*sin(pi/6)+3., x*sin(pi/6)+y*cos(pi/6)-2., z]
case('rotated_low', [rotate(p) for p in quad], [rotate([0., .75, 0.])])
case('rotated_high', [rotate(p) for p in quad], [rotate([0., 2.25, 0.])])
warped = deepcopy(quad)
warped[2][2] = 1.2
warped[3][2] = .4
case('warped_low', warped, [[0., .75, .1]])
case('warped_high', warped, [[0., 2.25, .3]])
case('skewed', [[0., 0., 0.], [6., 1., 0.], [4., 4., 0.], [-1., 3., 0.]], [[-.5, 1.5, 0.]])
