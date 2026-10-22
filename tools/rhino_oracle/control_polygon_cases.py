"""Fixed control-polygon sources and command options; no executable input."""
from copy import deepcopy

QUAD = [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
CUBE = QUAD + [[x, y, 2.] for x, y, _ in QUAD]
FACES = [[0, 3, 2, 1], [0, 1, 5, 4], [1, 2, 6, 5],
         [2, 3, 7, 6], [3, 0, 4, 7], [4, 5, 6, 7]]
SHAPES = {
    'line': dict(kind='line'),
    'circle': dict(kind='circle'),
    'polycurve': dict(kind='polycurve'),
    'surface': dict(kind='surface'),
    'sphere': dict(kind='sphere'),
    'trimmed': dict(kind='trimmed'),
    'box': dict(kind='box'),
    'extrusion': dict(kind='extrusion'),
    'point': dict(kind='point'),
    'mesh': dict(kind='mesh', vertices=QUAD, faces=[[0, 1, 2, 3]]),
    'subd_quad': dict(kind='subd', vertices=QUAD, faces=[[0, 1, 2, 3]]),
    'subd_cube': dict(kind='subd', vertices=CUBE, faces=FACES),
    'subd_crease': dict(kind='subd', vertices=CUBE, faces=FACES,
                        creases=[[0, 1], [1, 2], [2, 3], [0, 3]]),
    'subd_triangle': dict(kind='subd', vertices=QUAD[:3], faces=[[0, 1, 2]]),
    'subd_ngon': dict(kind='subd', vertices=QUAD + [[-2., 1.5, 0.]],
                      faces=[[0, 1, 2], [0, 2, 3], [0, 3, 4]],
                      ngons=[dict(vertices=[0, 1, 2, 3, 4], faces=[0, 1, 2])]),
}
CASES = {}
for name, source in SHAPES.items():
    CASES[name] = dict(sources=[deepcopy(source)], layer='Current', preselected=True)
for name in ['subd_cube', 'surface', 'trimmed', 'box', 'polycurve', 'point', 'mesh']:
    for layer in ['Input', 'TargetObject']:
        CASES[name + '_' + layer.lower()] = dict(sources=[deepcopy(SHAPES[name])],
                                                 layer=layer, preselected=True)
for name in ['subd_cube', 'surface', 'box', 'point', 'mesh']:
    CASES[name + '_postselected'] = dict(sources=[deepcopy(SHAPES[name])],
                                        layer='Current', preselected=False)
for name, kinds in [('multiple', ['subd_quad', 'subd_cube']),
                    ('mixed', ['subd_cube', 'line', 'surface']),
                    ('mixed_unsupported', ['point', 'mesh', 'subd_cube'])]:
    CASES[name] = dict(sources=[deepcopy(SHAPES[k]) for k in kinds], layer='Current', preselected=True)

OPTIONS = {}
for name in ['subd_quad', 'subd_cube', 'subd_crease', 'subd_ngon', 'surface']:
    for priority in ['Geometry', 'TextureCoordinates']:
        OPTIONS[name + '_' + priority.lower()] = dict(sources=[deepcopy(SHAPES[name])],
            layer='Current', priority=priority, preselected=True)
OPTIONS['default_after_texture'] = deepcopy(CASES['subd_cube'])
OPTIONS['target_separate_tokens'] = dict(sources=[deepcopy(SHAPES['subd_cube'])],
    layer='Current', tokens=['_OutputLayer', '_TargetObject'], preselected=True)
SCENES = dict(CASES, **OPTIONS)
PRIORITY = {}
for name, recipe in OPTIONS.items():
    if 'priority' in recipe:
        PRIORITY[name + '_picked'] = dict(deepcopy(recipe), preselected=False)
PRIORITY['priority_remembered_default'] = deepcopy(CASES['subd_cube'])
PRIORITY['target_picked'] = dict(deepcopy(OPTIONS['target_separate_tokens']), preselected=False)
SCENES.update(PRIORITY)

# Component and temporary-curve discovery is a separate immutable family.
COMPONENTS = {}
for name, shape, kind, indices, layer in [
    ('face_box_zero', 'box', 'BrepFace', [0], 'Current'),
    ('face_box_pair', 'box', 'BrepFace', [0, 2], 'Current'),
    ('face_box_reverse_pair', 'box', 'BrepFace', [2, 0], 'Current'),
    ('face_box_input', 'box', 'BrepFace', [1], 'Input'),
    ('face_trimmed', 'trimmed', 'BrepFace', [0], 'Current'),
    ('face_subd', 'subd_cube', 'SubdFace', [0], 'Current'),
    ('face_mesh', 'mesh', 'MeshFace', [0], 'Current'),
    ('segment_polycurve', 'polycurve', 'PolycurveSegment', [1], 'Current'),
]:
    COMPONENTS[name] = dict(sources=[deepcopy(SHAPES[shape])], layer=layer,
                            preselected=True, components=[[0, kind, i] for i in indices])
COMPONENTS['face_and_curve'] = dict(sources=[deepcopy(SHAPES['box']), deepcopy(SHAPES['line'])],
    layer='Current', preselected=True, components=[[0, 'BrepFace', 0]], whole=[1])
for name, shape, fractions in [
    ('range_line', 'line', [.2, .8]), ('range_polycurve', 'polycurve', [.1, .9]),
    ('range_circle', 'circle', [.1, .4]), ('range_circle_wrap', 'circle', [.8, .2]),
    ('range_spline', 'spline', [.2, .75]),
]:
    source = dict(kind='spline') if shape == 'spline' else deepcopy(SHAPES[shape])
    COMPONENTS[name] = dict(sources=[source], layer='Current', preselected=False, subcurve=fractions)
COMPONENTS['range_cancel'] = dict(deepcopy(COMPONENTS['range_line']), cancel=True)
CANCEL = {'range_cancel': COMPONENTS.pop('range_cancel')}
SCENES.update(COMPONENTS)
SCENES.update(CANCEL)
PICKS = {
    name: dict(sources=[deepcopy(SHAPES[shape])], layer='Current', preselected=False,
               mouse=dict(face=face, modifiers=modifiers))
    for name, shape, face, modifiers in [
        ('face_box_click', 'box', 5, 'sub'),
        ('face_box_plain_click', 'box', 5, 'plain'),
    ]
}
SCENES.update(PICKS)


def request():
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='control_polygon_command', id=name, case=name) for name in CASES])


def options_request():
    result = request()
    result['operations'] = [dict(op='control_polygon_command', id=name, case=name) for name in OPTIONS]
    return result


def priority_request():
    result = request()
    result['operations'] = [dict(op='control_polygon_command', id=name, case=name) for name in PRIORITY]
    return result


def components_request():
    result = request()
    result['operations'] = [dict(op='control_polygon_command', id=name, case=name) for name in COMPONENTS]
    return result


def picks_request():
    result = request()
    result['operations'] = [dict(op='control_polygon_command', id=name, case=name) for name in PICKS]
    return result


def validate(operation):
    try:
        valid = (isinstance(operation, dict) and set(operation) == {'op', 'id', 'case'}
                 and operation['op'] == 'control_polygon_command'
                 and operation['case'] in SCENES and operation['id'] == operation['case'])
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed control-polygon recipe required')
