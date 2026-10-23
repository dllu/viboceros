"""Independent mesh coverage, depth and view recipes for public outline APIs."""
from copy import deepcopy


def mesh(vertices, faces):
    return dict(vertices=deepcopy(vertices), faces=deepcopy(faces))


def joined(*parts):
    vertices, faces = [], []
    for part in parts:
        offset = len(vertices)
        vertices.extend(deepcopy(part['vertices']))
        faces.extend([[i + offset for i in face] for face in part['faces']])
    return mesh(vertices, faces)


SQUARE = mesh([[0., 0., 2.], [4., 0., 2.], [4., 3., 2.], [0., 3., 2.]], [[0, 1, 2, 3]])
BOX = mesh([[x, y, z] for z in [1., 4.] for y in [0., 3.] for x in [0., 4.]],
           [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4],
            [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]])
FRAME = mesh([[0., 0., 2.], [6., 0., 2.], [6., 6., 2.], [0., 6., 2.],
              [2., 2., 2.], [4., 2., 2.], [4., 4., 2.], [2., 4., 2.]],
             [[0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]])
SHAPES = {
    'triangle': mesh(SQUARE['vertices'][:3], [[0, 1, 2]]),
    'quad': SQUARE,
    'split': mesh(SQUARE['vertices'], [[0, 1, 2], [0, 2, 3]]),
    'box': BOX,
    'frame': FRAME,
    'warped': mesh([[0., 0., 1.], [4., 0., 1.], [4., 3., 4.], [0., 3., 1.]], [[0, 1, 2, 3]]),
    'concave': mesh([[0., 0., 2.], [4., 0., 2.], [1., 1., 2.], [0., 4., 2.]], [[0, 1, 2, 3]]),
    'edge_on': mesh([[0., 0., 0.], [4., 0., 0.], [2., 0., 3.]], [[0, 1, 2]]),
    'duplicate': mesh(SQUARE['vertices'], [[0, 1, 2, 3], [3, 2, 1, 0]]),
}
for name, dx, dy, dz in [('overlap', 2., 1., 2.), ('disconnected', 6., 1., 2.),
                          ('point_touch', 4., 3., 0.), ('edge_touch', 4., 0., 0.)]:
    other = mesh([[x + dx, y + dy, z + dz] for x, y, z in SQUARE['vertices']], SQUARE['faces'])
    SHAPES[name] = joined(SQUARE, other)
SHAPES['nested'] = joined(SQUARE, mesh([[1., 1., 8.], [2., 1., 8.], [2., 2., 8.]], [[0, 1, 2]]))
SHAPES['crossing'] = joined(mesh([[0., 0., 1.], [4., 0., 1.], [2., 4., 1.]], [[0, 1, 2]]),
                            mesh([[0., 3., 3.], [2., -1., 3.], [4., 3., 3.]], [[0, 1, 2]]))

PLANES = {
    'xy': dict(origin=[0., 0., 0.], x=[1., 0., 0.], y=[0., 1., 0.]),
    'offset': dict(origin=[3., -2., 9.], x=[1., 0., 0.], y=[0., 1., 0.]),
    'yz': dict(origin=[7., 0., 0.], x=[0., 1., 0.], y=[0., 0., 1.]),
    'tilted': dict(origin=[1., 2., 3.], x=[1., 0., 1.], y=[0., 1., 0.]),
}
SDK = {}
for name, shape in SHAPES.items():
    SDK['outline_plane_' + name] = dict(mesh=deepcopy(shape), method='plane', plane=deepcopy(PLANES['xy']))
for plane in ['offset', 'yz', 'tilted']:
    for shape in ['box', 'frame', 'warped']:
        SDK['outline_plane_' + shape + '_' + plane] = dict(
            mesh=deepcopy(SHAPES[shape]), method='plane', plane=deepcopy(PLANES[plane]))
for view in ['top', 'front', 'oblique', 'perspective']:
    for shape in ['box', 'frame', 'overlap', 'warped']:
        for method in ['viewport', 'info_plane']:
            name = 'outline_' + method + '_' + shape + '_' + view
            SDK[name] = dict(mesh=deepcopy(SHAPES[shape]), method=method, view=view,
                             plane=deepcopy(PLANES['xy']))
for view in ['top', 'oblique', 'perspective']:
    SDK['outline_info_plane_offset_' + view] = dict(
        mesh=deepcopy(BOX), method='info_plane', view=view, plane=deepcopy(PLANES['offset']))


PROJECTIONS = {}
for name, case in SDK.items():
    if case['method'] != 'plane':
        value = deepcopy(case)
        value['frustum'] = [-.5, .5, -.5, .5, 1., 80.]
        PROJECTIONS[name.replace('outline_', 'outline_projection_', 1)] = value
for view in ['oblique', 'perspective']:
    value = deepcopy(SDK['outline_viewport_box_' + view])
    value['frustum'] = [-1., 1., -1., 1., 2., 120.]
    PROJECTIONS['outline_projection_far_' + view] = value
ALL_CASES = dict(SDK, **PROJECTIONS)


def request(projections=False):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='mesh_outline_sdk', id=name, case=name)
                            for name in (PROJECTIONS if projections else SDK)])


def validate(operation):
    try:
        valid = (isinstance(operation, dict) and set(operation) == {'op', 'id', 'case'}
                 and operation['op'] == 'mesh_outline_sdk' and operation['case'] in ALL_CASES
                 and operation['id'] == operation['case'])
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed mesh outline recipe required')
