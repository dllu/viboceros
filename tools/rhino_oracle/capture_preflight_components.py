"""Boolean, UV, mesh/SubD component editing and result-history capture policy."""
from .oracle_errors import OracleProtocolError
from .capture_preflight_common import require_dedicated_capture


def prepare(client, request, interaction=None):
    if any(op.get('op') == 'subd_cap_regions' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_cap_regions',
            'SubD cap regions require a private dedicated one-iteration capture')
        from .subd_cap_regions_cases import validate
        for op in operations:validate(op)
    if any(op.get('op') == 'mesh_cap_regions' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'mesh_cap_regions',
            'Mesh cap regions require a private dedicated one-iteration capture')
        from .mesh_cap_regions_cases import validate
        for op in operations:validate(op)
    if any(op.get('op') == 'boolean_two_curved' for op in request.get('operations', [])):
        from .boolean_two_curved_probe import validate_request, SPECS
        if client.settings_scheme is None:
            raise OracleProtocolError('Curved Boolean2Objects requires a private settings scheme')
        validate_request(request)
        from .boolean_two_input import BooleanTwoPicker
        interaction=BooleanTwoPicker(request, validate_request, SPECS)
    if any(op.get('op') == 'boolean_split_topology' for op in request.get('operations', [])):
        from .boolean_split_topology_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanSplit topology requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_split_mixed_open' for op in request.get('operations', [])):
        from .boolean_split_mixed_open_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanSplit mixed open targets require a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_split_open' for op in request.get('operations', [])):
        from .boolean_split_open_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanSplit open targets require a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_split_plane' for op in request.get('operations', [])):
        from .boolean_split_plane_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanSplit plane probes require a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_split_command' for op in request.get('operations', [])):
        from .boolean_split_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanSplit requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_difference_order_command' for op in request.get('operations', [])):
        from .boolean_difference_order_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanDifference ordering requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_difference_command' for op in request.get('operations', [])):
        from .boolean_difference_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanDifference commands require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_intersection_command' for op in request.get('operations', [])):
        from .boolean_intersection_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanIntersection commands require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_union_command' for op in request.get('operations', [])):
        from .boolean_union_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('BooleanUnion commands require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'convex_boolean' for op in request.get('operations', [])):
        from .convex_boolean_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Convex Boolean probes require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'polyhedral_boolean' for op in request.get('operations', [])):
        from .polyhedral_boolean_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Polyhedral Boolean probes require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'compound_intersection' for op in request.get('operations', [])):
        from .compound_intersection_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Compound intersections require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'common_curved' for op in request.get('operations', [])):
        from .common_curved_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Curved common recipes require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'common_participation' for op in request.get('operations', [])):
        from .common_participation_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Common participation requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_curve_image' for op in request.get('operations', [])):
        from .surface_curve_image_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface curve image requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'apply_uv_curves_command' for op in request.get('operations', [])):
        from .apply_uv_curves_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('ApplyCrv requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'create_uv_curves_command' for op in request.get('operations', [])):
        from .create_uv_curves_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('CreateUVCrv requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') in ('mesh_face_collapse_buffers','mesh_face_collapse','mesh_face_collapse_point','mesh_face_collapse_click','mesh_face_collapse_precision','mesh_face_collapse_getter','mesh_face_collapse_shaded','mesh_face_collapse_mode') for op in request.get('operations', [])):
        operations = request['operations']
        if (client.settings_scheme is None or type(request.get('iterations', 1)) is not int
                or request.get('iterations', 1) != 1
                or not 1 <= len(operations) <= 64
                or len({op['op'] for op in operations}) != 1
                or any(op.get('op') not in ('mesh_face_collapse_buffers','mesh_face_collapse','mesh_face_collapse_point','mesh_face_collapse_click','mesh_face_collapse_precision','mesh_face_collapse_getter','mesh_face_collapse_shaded','mesh_face_collapse_mode') for op in operations)):
            raise OracleProtocolError('Face collapse requires a private dedicated one-iteration capture')
        if any(op.get('op') in ('mesh_face_collapse_getter','mesh_face_collapse_shaded','mesh_face_collapse_mode') for op in operations):
            from .catenary_point_picker import CatenaryPointPicker
            interaction = CatenaryPointPicker(dict(operations=[dict(id=op['id'],actions=[dict(click=True)]) for op in operations]))
        if any(op.get('op') == 'mesh_face_collapse_click' for op in operations):
            from .mesh_face_collapse_picker import FaceCollapsePicker
            interaction = FaceCollapsePicker()
    if any(op.get('op') == 'mesh_insert_conversion' for op in request.get('operations', [])):
        require_dedicated_capture(client, request, 'mesh_insert_conversion',
            'Mesh insertion conversion requires a private dedicated one-iteration capture')
    if any(op.get('op') == 'mesh_insert_validation' for op in request.get('operations', [])):
        require_dedicated_capture(client, request, 'mesh_insert_validation',
            'Mesh insertion validation requires a private dedicated one-iteration capture')
    if any(op.get('op') == 'mesh_insert_degenerate' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'mesh_insert_degenerate',
            'Flat-corner insertion requires a private dedicated one-iteration capture')
        from .mesh_insert_degenerate_cases import CASES,validate
        from .catenary_point_picker import CatenaryPointPicker
        for op in operations:validate(op)
        interaction=CatenaryPointPicker(dict(operations=[dict(id=op['id'],actions=CASES[op['case']]['actions'])for op in operations]))
    if any(op.get('op') == 'mesh_insert_paths' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'mesh_insert_paths',
            'Mesh insertion paths require a private dedicated one-iteration capture')
        from .mesh_insert_paths_cases import CASES,validate
        from .catenary_point_picker import CatenaryPointPicker
        for op in operations:validate(op)
        interaction=CatenaryPointPicker(dict(operations=[dict(id=op['id'],actions=CASES[op['case']]['actions'])for op in operations]))
    if any(op.get('op') == 'subd_insert_order' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_insert_order',
            'SubD insertion holdouts require a private dedicated one-iteration capture')
        from .subd_insert_order_cases import CASES,validate
        from .catenary_point_picker import CatenaryPointPicker
        for op in operations:validate(op)
        interaction = CatenaryPointPicker(dict(operations=[dict(id=op['id'],actions=CASES[op['case']]['actions'])for op in operations]))
    if any(op.get('op') == 'subd_insert_custom' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_insert_custom',
            'SubD custom insertion requires a private dedicated one-iteration capture')
        from .subd_insert_custom_cases import CASES,validate
        from .catenary_point_picker import CatenaryPointPicker
        for op in operations:validate(op)
        interaction = CatenaryPointPicker(dict(operations=[dict(id=op['id'],actions=CASES[op['case']]['actions'])for op in operations]))
    if any(op.get('op') == 'subd_face_custom_holdout' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_face_custom_holdout',
            'SubD custom-face holdouts require a private dedicated one-iteration capture')
    if any(op.get('op') == 'subd_face_custom' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_face_custom',
            'SubD custom faces require a private dedicated one-iteration capture')
    if any(op.get('op') == 'subd_result_history' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_result_history',
            'SubD result history requires a private dedicated one-iteration capture')
    if any(op.get('op') == 'subd_merge_allocation' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_merge_allocation',
            'SubD allocation requires a private dedicated one-iteration capture')
    if any(op.get('op') == 'subd_merge_properties' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_merge_properties',
            'SubD properties require a private dedicated one-iteration capture')
    if any(op.get('op') == 'merge_faces_command' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'merge_faces_command',
            'MergeFaces requires a private dedicated one-iteration capture')
        from .merge_faces_command_cases import EXTRA_CASES
        if any(EXTRA_CASES.get(op['case'], {}).get('postselect') for op in operations):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get('op') == 'subd_merge_faces' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'subd_merge_faces',
            'SubD merging requires a private dedicated one-iteration capture')
    if any(op.get('op') == 'mesh_merge_faces' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'mesh_merge_faces',
            'Mesh merging requires a private dedicated one-iteration capture')
    if any(op.get('op') in ('pt_off_selected','pt_off_selected_history') for op in request.get('operations', [])):
        operations = request['operations']
        if (client.settings_scheme is None or type(request.get('iterations', 1)) is not int
                or request.get('iterations', 1) != 1 or not 1 <= len(operations) <= 64
                or len({op.get('op') for op in operations}) != 1
                or any(op.get('op') not in ('pt_off_selected','pt_off_selected_history') for op in operations)):
            raise OracleProtocolError('Selected point display requires a private dedicated one-iteration capture')
    if any(op.get('op') == 'typed_selection' for op in request.get('operations', [])):
        operations = require_dedicated_capture(client, request, 'typed_selection',
            'Typed selection commands require a private dedicated one-iteration capture', strict_iterations=False)
    return interaction
