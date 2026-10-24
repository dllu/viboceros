"""Fixed independent operands for the public polygon Boolean SDK."""
from copy import deepcopy
import math


def box(origin=(0., 0., 0.), size=(4., 3., 2.)):
    vertices = [[origin[0]+x*size[0], origin[1]+y*size[1], origin[2]+z*size[2]]
                for z in [0, 1] for y in [0, 1] for x in [0, 1]]
    return dict(vertices=vertices, faces=[[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4],
                                         [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]])


PAIRS = {
    'overlap': [box(), box((2., 1., .5))],
    'disjoint': [box(), box((8., 1., .5))],
    'contained': [box(), box((1., 1., .5), (1., 1., 1.))],
    'identical': [box(), box()],
    'face_touch': [box(), box((4., 0., 0.))],
    'edge_touch': [box(), box((4., 3., 0.))],
    'point_touch': [box(), box((4., 3., 2.))],
    'cavity': [box(size=(6., 6., 6.)), box((1., 1., 1.), (4., 4., 4.))],
}
rotated = box((2., -.5, .5))
angle = .37
rotated['vertices'] = [[math.cos(angle)*x-math.sin(angle)*y,
                        math.sin(angle)*x+math.cos(angle)*y, z] for x, y, z in rotated['vertices']]
PAIRS['rotated'] = [box(), rotated]
unwelded = deepcopy(PAIRS['overlap'])
for mesh in unwelded:
    vertices = [mesh['vertices'][i] for face in mesh['faces'] for i in face]
    mesh['vertices'] = vertices
    mesh['faces'] = [list(range(i, i+4)) for i in range(0, len(vertices), 4)]
PAIRS['unwelded'] = unwelded

SDK = {name+'_'+operation: dict(meshes=deepcopy(meshes), operation=operation)
       for name, meshes in PAIRS.items() for operation in ['union', 'intersection', 'difference']}
CONTROLS = {}
for repetition in range(3):
    for operation in ['union', 'intersection', 'difference']:
        CONTROLS['coincident_repeat_{}_{}'.format(repetition, operation)] = deepcopy(SDK['identical_'+operation])
for variant in ['triangles', 'cycled', 'face_touch']:
    for operation in ['union', 'intersection', 'difference']:
        value = deepcopy(SDK[('face_touch' if variant == 'face_touch' else 'identical')+'_'+operation])
        for mesh in value['meshes']:
            if variant == 'triangles':
                mesh['faces'] = [[f[0], f[1], f[2]] for f in mesh['faces']] + [[f[0], f[2], f[3]] for f in mesh['faces']]
            elif variant == 'cycled':
                mesh['faces'] = [f[1:]+f[:1] for f in mesh['faces']]
        CONTROLS['coincident_{}_{}'.format(variant, operation)] = value
ALL_CASES = dict(SDK, **CONTROLS)


def request(controls=False):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='mesh_boolean_sdk', id=name, case=name) for name in (CONTROLS if controls else SDK)])


def validate(operation):
    valid = (isinstance(operation, dict) and set(operation) == {'op', 'id', 'case'}
             and operation.get('op') == 'mesh_boolean_sdk'
             and isinstance(operation.get('case'), str) and operation['case'] in ALL_CASES
             and operation['id'] == operation['case'])
    if not valid:
        raise ValueError('fixed mesh Boolean recipe required')
