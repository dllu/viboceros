"""Curve construction, surface rebuilding and two-set Boolean capture policy."""
from .oracle_errors import OracleProtocolError


def prepare(client, request, interaction=None):
    if any(op.get('op') in ('circle_around','circle_deform','circle_tangent') for op in request.get('operations', [])):
        from .circle_around_probe import validate
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')not in('circle_around','circle_deform','circle_tangent')for op in operations):
            raise OracleProtocolError('Circle getters require a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('Circle getters require 1 through 64 uniquely named cases')
        for op in operations:validate(op)
        if any(op.get('op')=='circle_tangent' for op in operations):
            if any(op.get('op')!='circle_tangent' for op in operations):raise OracleProtocolError('Circle Tangent requires a dedicated contact-picking capture')
            from .catenary_point_picker import CatenaryPointPicker
            click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in op['actions']])for op in operations])
            interaction=CatenaryPointPicker(click_request)
        if not any(op.get('op')=='circle_tangent'for op in operations)and any(isinstance(a,dict)and'edge'in a for op in operations for a in op.get('actions',[])):
            from .conic_getter_picker import ConicGetterPicker
            interaction=ConicGetterPicker(request)
    if any(op.get('op') == 'ellipse_command' for op in request.get('operations', [])):
        from .ellipse_probe import validate
        operations = request['operations']
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1 or any(op.get('op') != 'ellipse_command' for op in operations):
            raise OracleProtocolError('Ellipse getters require a private, dedicated one-iteration capture')
        names = [op.get('id') for op in operations]
        if not 1 <= len(operations) <= 64 or any(not isinstance(name, str) for name in names) or len(set(names)) != len(names):
            raise OracleProtocolError('Ellipse getters require 1 through 64 uniquely named cases')
        for operation in operations:
            validate(operation)
        if any(isinstance(a,dict) and 'edge' in a for op in operations for a in op.get('actions',[])):
            from .conic_getter_picker import ConicGetterPicker
            interaction=ConicGetterPicker(request)
    if any(op.get('op') == 'catenary_command' for op in request.get('operations', [])):
        from .catenary_command_probe import validate
        operations = request['operations']
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1 or any(op.get('op') != 'catenary_command' for op in operations):
            raise OracleProtocolError('Catenary getters require a private dedicated one-iteration request')
        if not 1 <= len(operations) <= 64 or len({op.get('id') for op in operations}) != len(operations):
            raise OracleProtocolError('Catenary getters require one through 64 uniquely named cases')
        for operation in operations:
            validate(operation)
        if any(isinstance(a,dict) and 'click' in a for op in operations for a in op.get('actions',[])):
            from .catenary_point_picker import CatenaryPointPicker
            interaction = CatenaryPointPicker(request)
    if any(op.get('op')in('helix_command','spiral_command') and op.get('live_getters') for op in request.get('operations',[])):
        operations=request.get('operations',[])
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')not in('helix_command','spiral_command')for op in operations):
            raise OracleProtocolError('live spiral getters require a private scheme and a dedicated one-iteration request')
        if not 1<=len(operations)<=128 or len({op.get('id')for op in operations})!=len(operations):
            raise OracleProtocolError('live spiral getters require one through 128 unique cases')
        if any(isinstance(a,dict)and'edge'in a for op in operations for a in op.get('actions',[])):
            from .conic_getter_picker import ConicGetterPicker
            interaction=ConicGetterPicker(request)
    if any(op.get('op')=='hyperbola_command' and op.get('live_getters') for op in request.get('operations',[])):
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='hyperbola_command'for op in operations):
            raise OracleProtocolError('Hyperbola live input requires a private dedicated one-iteration request')
        if not 1<=len(operations)<=128 or len({op['id']for op in operations})!=len(operations):
            raise OracleProtocolError('Hyperbola live input requires uniquely named bounded cases')
    if any(op.get('op')=='conic_command' and any(isinstance(a,dict)and('hover'in a or'edge'in a)for a in op.get('actions',[]))for op in request.get('operations',[])):
        if client.settings_scheme is None or request.get('iterations',1)!=1 or any(op.get('op')!='conic_command'for op in request.get('operations',[])):
            raise OracleProtocolError('Conic hover input requires a private dedicated one-iteration request')
        operations=request['operations']
        if not 1<=len(operations)<=128 or len({o['id']for o in operations})!=len(operations):raise OracleProtocolError('Conic hover input requires uniquely named bounded cases')
        from .conic_getter_picker import ConicGetterPicker
        interaction=ConicGetterPicker(request)
    if any(op.get('op')=='surface_flow' and op.get('mouse') for op in request.get('operations', [])):
        if client.settings_scheme is None:
            raise OracleProtocolError('surface Flow mouse capture requires a private settings scheme')
        if any(op.get('op') != 'surface_flow' for op in request.get('operations', [])):
            raise OracleProtocolError('surface Flow mouse capture requires a dedicated request')
        from .group_picking import IdlePicker
        interaction = IdlePicker()
    if any(op.get('op') == 'surface_retrim_profile' for op in request.get('operations', [])):
        from .surface_retrim_profile_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Retrim profiling requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_singular_trim' for op in request.get('operations', [])):
        from .surface_rebuild_singular_trim_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_crossing_hole' for op in request.get('operations', [])):
        from .surface_rebuild_crossing_hole_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_seam_trim' for op in request.get('operations', [])):
        from .surface_rebuild_seam_trim_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_closed_degrees' for op in request.get('operations', [])):
        from .surface_rebuild_closed_degrees_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_closed' for op in request.get('operations', [])):
        from .surface_rebuild_closed_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_natural' for op in request.get('operations', [])):
        from .surface_rebuild_natural_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_retrim_followup' for op in request.get('operations', [])):
        from .surface_rebuild_retrim_followup_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_retrim' for op in request.get('operations', [])):
        from .surface_rebuild_retrim_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_option_followup' for op in request.get('operations', [])):
        from .surface_rebuild_option_followup_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild_options' for op in request.get('operations', [])):
        from .surface_rebuild_options_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_rebuild' for op in request.get('operations', [])):
        from .surface_rebuild_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Surface Rebuild requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_control_followup' for op in request.get('operations', [])):
        from .tween_surfaces_control_followup_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('TweenSurfaces requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_control_rows' for op in request.get('operations', [])):
        from .tween_surfaces_control_rows_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('TweenSurfaces requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_control' for op in request.get('operations', [])):
        from .tween_surfaces_control_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('TweenSurfaces requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_corner_sequences' for op in request.get('operations', [])):
        from .tween_surfaces_corner_sequences_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
        from .tween_surfaces_corner_sequence_input import SequencePicker
        interaction=SequencePicker(request)
    if any(op.get('op') == 'tween_surfaces_corners' for op in request.get('operations', [])):
        from .tween_surfaces_corners_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
        from .tween_surfaces_corner_input import CornerPicker
        interaction=CornerPicker(request)
    if any(op.get('op') == 'tween_surfaces_sample_memory' for op in request.get('operations', [])):
        from .tween_surfaces_sample_memory_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_options' for op in request.get('operations', [])):
        from .tween_surfaces_options_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_interaction' for op in request.get('operations', [])):
        from .tween_surfaces_interaction_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_refit' for op in request.get('operations', [])):
        from .tween_surfaces_refit_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_sampling' for op in request.get('operations', [])):
        from .tween_surfaces_sampling_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'tween_surfaces_command' for op in request.get('operations', [])):
        from .tween_surfaces_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'planar_circle_scale' for op in request.get('operations', [])):
        from .planar_circle_scale_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'planar_boolean_scale' for op in request.get('operations', [])):
        from .planar_boolean_scale_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'planar_boolean_mixed' for op in request.get('operations', [])):
        from .planar_boolean_mixed_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'planar_boolean_circular' for op in request.get('operations', [])):
        from .planar_boolean_circular_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'planar_boolean_topology' for op in request.get('operations', [])):
        from .planar_boolean_topology_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'planar_boolean_command' for op in request.get('operations', [])):
        from .planar_boolean_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Planar Boolean requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'boolean_two_coplanar' for op in request.get('operations', [])):
        from .boolean_two_coplanar_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Boolean2Objects requires a private settings scheme')
        validate_request(request)
        from .boolean_two_coplanar_input import BooleanTwoCoplanarPicker
        interaction=BooleanTwoCoplanarPicker(request)
    if any(op.get('op') == 'boolean_two_open' for op in request.get('operations', [])):
        from .boolean_two_open_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Boolean2Objects requires a private settings scheme')
        validate_request(request)
        from .boolean_two_open_input import BooleanTwoOpenPicker
        interaction=BooleanTwoOpenPicker(request)
    if any(op.get('op') == 'boolean_two_command' for op in request.get('operations', [])):
        from .boolean_two_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Boolean2Objects requires a private settings scheme')
        validate_request(request)
        from .boolean_two_input import BooleanTwoPicker
        interaction=BooleanTwoPicker(request)
    return interaction
