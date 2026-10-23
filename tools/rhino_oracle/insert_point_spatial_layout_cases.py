"""Independent two-pick quad layouts with spatial and boundary-order variations."""
from math import cos, sin, pi
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source

CASES = {}
QUAD = [[0., 0., 0.], [4., 0., 0.], [4., 3., 1.2], [0., 3., .4]]


def case(name, vertices, picks, face=None):
    def point(edge, parameter):
        a, b = vertices[edge], vertices[(edge+1) % 4]
        return [x+(y-x)*parameter for x, y in zip(a, b)]
    points = [point(edge, parameter) for edge, parameter in picks]
    CASES['insert_point_spatial_'+name] = dict(command='InsertPoint',
        sources=[source('mesh', vertices=[list(p) for p in vertices], faces=[face or [0, 1, 2, 3]])],
        whole=True, preselected=True, live=True, mesh_channels=False,
        points=points, actions=[dict(click=p) for p in points]+['Enter'])


case('bottom_low_left_low', QUAD, [(0, .25), (3, .75)])
case('bottom_high_left_low', QUAD, [(0, .75), (3, .75)])
case('bottom_mid_left_high', QUAD, [(0, .5), (3, .25)])
case('left_low_bottom_mid', QUAD, [(3, .75), (0, .5)])
case('mild_warp', [[x, y, z*.25] for x, y, z in QUAD], [(0, .5), (3, .75)])
case('saddle', [[0., 0., 0.], [4., 0., 0.], [4., 3., 1.2], [0., 3., -.4]], [(0, .5), (3, .75)])
case('tilted_plane', [[x, y, .3*x+.2*y] for x, y, _ in QUAD], [(0, .5), (3, .75)])
angle = pi/6
case('rotated_warp', [[x*cos(angle)-y*sin(angle)+3., x*sin(angle)+y*cos(angle)-2., z]
                      for x, y, z in QUAD], [(0, .5), (3, .75)])
case('tall_warp', [[0., 0., 0.], [2., 0., 0.], [2., 6., 1.2], [0., 6., .4]], [(0, .5), (3, .75)])
case('reversed_warp', QUAD, [(0, .5), (3, .75)], [0, 3, 2, 1])
case('cyclic_warp', QUAD, [(0, .5), (3, .75)], [1, 2, 3, 0])
case('same_edge_warp', QUAD, [(3, .75), (3, .25)])
