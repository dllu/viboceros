"""Fixed public MeshTrim recipes on owned mesh scenes."""
from copy import deepcopy
if __package__:
    from .mesh_split_cases import box, xy, wall, moved, CHANNELS
    from .mesh_trim_control_cases import cases as control_cases
    from .mesh_trim_continuation_cases import cases as continuation_cases
    from .mesh_trim_snapshot_policy_cases import cases as snapshot_cases
    from .mesh_trim_cutter_policy_cases import cases as cutter_policy_cases
    from .mesh_trim_curve_control_cases import cases as curve_control_cases
    from .mesh_trim_pick_control_cases import cases as pick_control_cases
    from .mesh_trim_getter_control_cases import cases as getter_control_cases
    from .mesh_trim_getter_control_cases import plane_cases
else:
    from mesh_split_cases import box, xy, wall, moved, CHANNELS
    from mesh_trim_control_cases import cases as control_cases
    from mesh_trim_continuation_cases import cases as continuation_cases
    from mesh_trim_snapshot_policy_cases import cases as snapshot_cases
    from mesh_trim_cutter_policy_cases import cases as cutter_policy_cases
    from mesh_trim_curve_control_cases import cases as curve_control_cases
    from mesh_trim_pick_control_cases import cases as pick_control_cases
    from mesh_trim_getter_control_cases import cases as getter_control_cases
    from mesh_trim_getter_control_cases import plane_cases

DISCOVERY = {}
for name, shape, pre, extra in [
        ('box_id_post', box, False, {}),
        ('box_id_pre', box, True, {}),
        ('sheet_id_post', xy, False, {}),
        ('sheet_point_post', xy, False, dict(second=[], second_arguments='1,1,0')),
        ('cancel_first', xy, False, dict(cancel=True)),
        ('cancel_second', xy, False, dict(cancel_second=True)),
        ('tolerance_menu', xy, False, dict(arguments='_Tolerance _Cancel', cancel=True)),
        ('plane_menu', xy, False, dict(arguments='_IP _WorldYZ', cancel=True)),
]:
    value = dict(operation='split', preselect=pre, sources=deepcopy([shape, wall]),
                 first=[1], second=[0], capture_split_topology=True, mesh_channels=True)
    value.update(extra)
    DISCOVERY['mesh_trim_' + name] = value
MOUSE = {}
for name, shape, pre, point in [
        ('sheet_left_post', xy, False, [1., 1., 0.]),
        ('sheet_right_post', xy, False, [3., 1., 0.]),
        ('sheet_left_pre', xy, True, [1., 1., 0.]),
        ('box_left_post', box, False, [1., 1., 2.]),
]:
    identifier = 'mesh_trim_mouse_' + name
    MOUSE[identifier] = dict(operation='split', preselect=pre, sources=deepcopy([shape, wall]),
                            first=[1], second=[], trim_picks=[point], capture_id=identifier,
                            capture_split_topology=True, mesh_channels=True)
CONTROLS=control_cases(box,xy,wall,moved,CHANNELS)
FAMILIES = dict(discovery=DISCOVERY, mouse=MOUSE, controls=CONTROLS)
CONTINUATION = continuation_cases(box, xy, wall, moved)
FAMILIES['continuation'] = CONTINUATION
SNAPSHOT_POLICY = snapshot_cases(xy)
FAMILIES['snapshot_policy'] = SNAPSHOT_POLICY
CUTTER_POLICY = cutter_policy_cases(xy)
FAMILIES['cutter_policy'] = CUTTER_POLICY
CURVE_CONTROLS = curve_control_cases(xy, wall)
FAMILIES['curve_controls'] = CURVE_CONTROLS
PICK_CONTROLS = pick_control_cases(CURVE_CONTROLS)
FAMILIES['pick_controls'] = PICK_CONTROLS
SETTLED_PICKS = {}
for name, value in PICK_CONTROLS.items():
    identifier = name.replace('mesh_trim_pick_controls_', 'mesh_trim_settled_picks_')
    value = deepcopy(value)
    value.update(capture_id=identifier, trace_objects=False)
    SETTLED_PICKS[identifier] = value
FAMILIES['settled_picks'] = SETTLED_PICKS
DIRECT_GETTER = {}
for name, value in SETTLED_PICKS.items():
    if name.endswith('curve_only_quad'):
        continue
    identifier = name.replace('mesh_trim_settled_picks_', 'mesh_trim_direct_getter_')
    value = deepcopy(value)
    value.update(capture_id=identifier, direct_getter=True)
    DIRECT_GETTER[identifier] = value
FAMILIES['direct_getter'] = DIRECT_GETTER
GETTER_CONTROLS = getter_control_cases(CURVE_CONTROLS, MOUSE, wall)
FAMILIES['getter_controls'] = GETTER_CONTROLS
VERIFIED_GETTERS = {}
for name, value in GETTER_CONTROLS.items():
    if not value['preselect']:
        continue
    identifier = name.replace('mesh_trim_getter_controls_', 'mesh_trim_verified_getters_')
    value = deepcopy(value)
    value.pop('direct_getter')
    value['capture_id'] = identifier
    VERIFIED_GETTERS[identifier] = value
FAMILIES['verified_getters'] = VERIFIED_GETTERS
PLANE_GETTERS = plane_cases(CONTROLS, xy, moved)
FAMILIES['plane_getters'] = PLANE_GETTERS
ALL_CASES = {name: case for family in FAMILIES.values() for name, case in family.items()}


def request(family='discovery'):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='mesh_trim_command', id=name, case=name) for name in FAMILIES[family]])


def validate(operation):
    if not (isinstance(operation, dict) and set(operation) == {'op', 'id', 'case'} and
            operation.get('op') == 'mesh_trim_command' and
            isinstance(operation.get('case'), str) and operation['case'] in ALL_CASES and
            operation['id'] == operation['case']):
        raise ValueError('fixed MeshTrim command recipe required')
