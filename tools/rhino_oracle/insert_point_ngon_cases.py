"""Independent n-gon membership probes across original and newly tessellated edges."""
from copy import deepcopy
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source

CASES = {}


def case(name, vertices, faces, ngons, points):
    shape = source('mesh', vertices=deepcopy(vertices), faces=deepcopy(faces))
    shape['ngons'] = deepcopy(ngons)
    CASES['insert_point_ngon_'+name] = dict(
        command='InsertPoint', sources=[shape], whole=True, preselected=True,
        live=True, mesh_channels=False, points=points,
        actions=[dict(click=p) for p in points]+['Enter'])


triangle = [[0., 0., 0.], [4., 0., 0.], [0., 3., 0.]]
case('triangle_uncovered', triangle, [[0, 1, 2]], [], [[0., 1.5, 0.]])
case('triangle_overlay', triangle, [[0, 1, 2]],
     [dict(vertices=[0, 1, 2], faces=[0])], [[0., 1.5, 0.]])
case('triangle_same_edge', triangle, [[0, 1, 2]], [], [[0., .75, 0.], [0., 2.25, 0.]])

quad = [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
ngon = [dict(vertices=[0, 1, 2, 3], faces=[0, 1])]
case('quad_bottom', quad, [[0, 1, 2], [0, 2, 3]], ngon, [[2., 0., 0.]])
case('quad_reordered', quad, [[0, 2, 3], [0, 1, 2]], ngon, [[0., 1.5, 0.]])
case('quad_same_edge', quad, [[0, 1, 2], [0, 2, 3]], ngon,
     [[0., .75, 0.], [0., 2.25, 0.]])

hexagon = [[4., 0., 0.], [2., 3.464, 0.], [-2., 3.464, 0.],
           [-4., 0., 0.], [-2., -3.464, 0.], [2., -3.464, 0.], [0., 0., 0.]]
fan = [[6, i, (i+1) % 6] for i in range(6)]
hex_ngon = [dict(vertices=list(range(6)), faces=list(range(6)))]
case('fan_boundary', hexagon, fan, hex_ngon, [[-3., 1.732, 0.]])
case('fan_internal', hexagon, fan, hex_ngon, [[2., 0., 0.]])
case('fan_uncovered', hexagon, fan, [], [[2., 0., 0.]])
case('fan_cross', hexagon, fan, hex_ngon, [[-3., 1.732, 0.], [3., 1.732, 0.]])

strip = [[0., 0., 0.], [2., 0., 0.], [4., 0., 0.],
         [0., 3., 0.], [2., 3., 0.], [4., 3., 0.]]
case('two_ngons_shared', strip, [[0, 1, 4], [0, 4, 3], [1, 2, 5], [1, 5, 4]],
     [dict(vertices=[0, 1, 4, 3], faces=[0, 1]), dict(vertices=[1, 2, 5, 4], faces=[2, 3])],
     [[2., 1.5, 0.]])
case('two_overlays_shared', strip, [[0, 1, 4, 3], [1, 2, 5, 4]],
     [dict(vertices=[0, 1, 4, 3], faces=[0]), dict(vertices=[1, 2, 5, 4], faces=[1])],
     [[2., 1.5, 0.]])
