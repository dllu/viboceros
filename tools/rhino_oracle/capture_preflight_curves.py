"""Subcurve, surface pullback and transform-preview capture policy."""
from .oracle_errors import OracleProtocolError


def prepare(client, request, interaction=None):
    if any(op.get('op') in ('reparameterize_face_command','reparameterize_block_command') for op in request.get('operations', [])):
        if client.settings_scheme is None or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Face reparameterization commands require a private Rhino settings scheme and one iteration')
    if any(op.get('op') == 'uv_face_reference_command' for op in request.get('operations', [])):
        from .uv_face_reference_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('UV face references require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'uv_subcurve_input_command' for op in request.get('operations', [])):
        from .uv_subcurve_input_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('UV SubCrv requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'subcurve_numeric_followup' for op in request.get('operations', [])):
        from .subcurve_numeric_followup_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('numeric followup requires a private Rhino settings scheme')
        validate_request(request)
        from .group_picking import IdlePicker
        interaction=IdlePicker()
    if any(op.get('op') == 'standalone_subcurve' for op in request.get('operations', [])):
        from .standalone_subcurve_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('standalone SubCrv requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'subcurve_mark_ends' for op in request.get('operations', [])):
        from .subcurve_mark_ends_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('MarkEnds requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'subcurve_midpoint' for op in request.get('operations', [])):
        from .subcurve_midpoint_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('FromMidpoint requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'subcurve_preferences' for op in request.get('operations', [])):
        from .subcurve_preferences_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('SubCrv preferences require a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'subcurve_direction' for op in request.get('operations', [])):
        from .subcurve_direction_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('direction locking requires a private settings scheme')
        validate_request(request)
        from .subcurve_direction_input import SubcurveDirectionPicker
        interaction=SubcurveDirectionPicker(request)
    if any(op.get('op') == 'insert_kink_subcurve' and op.get('getter', {}).get('cancel_getter') for op in request.get('operations', [])):
        if client.settings_scheme is None:
            raise OracleProtocolError('temporary kink direction input requires a private settings scheme')
        from .insert_kink_subcurve_input import InsertKinkSubcurvePicker
        interaction=InsertKinkSubcurvePicker(request)
    if any(op.get('op') == 'subcurve_direction_grid' for op in request.get('operations', [])):
        from .subcurve_direction_grid_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('direction matrix requires a private settings scheme')
        validate_request(request)
        from .subcurve_direction_grid_input import DirectionGridPicker
        interaction=DirectionGridPicker(request)
    if any(op.get('op') == 'orient_on_curve' and 'edge' in op for op in request.get('operations', [])):
        if client.settings_scheme is None or request.get('iterations',1)!=1 or any(op.get('op')!='orient_on_curve' for op in request.get('operations',[])):
            raise OracleProtocolError('edge orientation input requires a private dedicated one-iteration request')
        from .orient_on_curve_probe import validate
        operations=request['operations']
        if not 1<=len(operations)<=128 or len({op['id']for op in operations})!=len(operations):
            raise OracleProtocolError('edge orientation requires 1 to 128 uniquely named cases')
        for op in operations:validate(op)
        from .orient_on_curve_edge_picker import OrientCurveEdgePicker
        interaction=OrientCurveEdgePicker(request)
    if any(op.get('op') == 'subcurve_edge' for op in request.get('operations', [])):
        from .subcurve_edge_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('edge SubCrv requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_pullback_endpoints' for op in request.get('operations', [])):
        from .surface_pullback_endpoints_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Pullback endpoints requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_pullback_linear' for op in request.get('operations', [])):
        from .surface_pullback_linear_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Linear pullback requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'surface_pullback_interpolation' for op in request.get('operations', [])):
        from .surface_pullback_interpolation_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Interpolated pullback requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'compound_pairs' for op in request.get('operations', [])):
        from .compound_pairs_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Compound pairs require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'polyhedral_boolean_command' for op in request.get('operations', [])):
        from .polyhedral_command_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Polyhedral commands require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'grip_alias' for op in request.get('operations', [])):
        from .grip_alias_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Grip aliases require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'smooth_workflow' for op in request.get('operations', [])):
        from .smooth_workflow_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Smooth workflows require a private Rhino settings scheme')
        validate_request(request)
        from .smooth_workflow_input import SmoothKeyboard
        interaction = SmoothKeyboard(request)
    if any(op.get('op') == 'smooth_uvn' for op in request.get('operations', [])):
        from .smooth_uvn_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Smooth UVN requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'smooth_frames' for op in request.get('operations', [])):
        from .smooth_frames_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Smooth direction witnesses require a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'smooth_command' for op in request.get('operations', [])):
        from .smooth_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Smooth requires a private Rhino settings scheme')
        validate_request(request)
    if any(op.get('op') == 'scale_by_plane_curve' for op in request.get('operations', [])):
        from .scale_by_plane_curve_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('ScaleByPlane curve frames require a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'scale_by_plane_object' for op in request.get('operations', [])):
        from .scale_by_plane_object_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('ScaleByPlane Object requires a private settings scheme')
        validate_request(request)
        from .scale_by_plane_object_input import ScaleByPlaneObjectPicker
        interaction = ScaleByPlaneObjectPicker(request)
    if any(op.get('op') == 'scale_by_plane' for op in request.get('operations', [])):
        from .scale_by_plane_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('ScaleByPlane requires a private settings scheme')
        validate_request(request)
        from .scale_by_plane_input import ScaleByPlaneViewPicker
        interaction = ScaleByPlaneViewPicker(request)
    if any(op.get('op') == 'point_input_precision' for op in request.get('operations', [])):
        from .point_input_precision_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('Point input precision requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'scale_positions_cursor' for op in request.get('operations', [])):
        from .scale_positions_cursor_input import ScalePositionsCursorPicker
        if client.settings_scheme is None:
            raise OracleProtocolError('ScalePositions cursor requires a private settings scheme')
        interaction = ScalePositionsCursorPicker(request)
    if any(op.get('op') == 'scale_positions' for op in request.get('operations', [])):
        from .scale_positions_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('ScalePositions requires a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'scale_nu_options' for op in request.get('operations', [])):
        from .scale_nu_options_probe import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError('ScaleNU options require a private settings scheme')
        validate_request(request)
    if any(op.get('op') == 'scale_nu_reference' for op in request.get('operations', [])):
        from .scale_nu_reference_input import ScaleNuReferencePicker
        if client.settings_scheme is None:
            raise OracleProtocolError('ScaleNU references require a private settings scheme')
        interaction = ScaleNuReferencePicker(request)
    if any(op.get('op') == 'maelstrom_preview' for op in request.get('operations', [])):
        from .maelstrom_preview_input import MaelstromPreviewPicker
        if client.settings_scheme is None:
            raise OracleProtocolError('Maelstrom previews require a private settings scheme')
        interaction=MaelstromPreviewPicker(request)
    if any(op.get('op') == 'taper_preview' for op in request.get('operations', [])):
        from .taper_preview_input import TaperPreviewPicker
        if client.settings_scheme is None:
            raise OracleProtocolError('Taper previews require a private settings scheme')
        interaction=TaperPreviewPicker(request)
    if any(op.get('op') == 'bend_preview' for op in request.get('operations', [])):
        from .bend_preview_input import BendPreviewPicker
        if client.settings_scheme is None:
            raise OracleProtocolError('Bend previews require a private settings scheme')
        interaction = BendPreviewPicker(request)
    if any(op.get('op') == 'twist_preview' for op in request.get('operations', [])):
        from .twist_preview_input import TwistPreviewPicker
        if client.settings_scheme is None:
            raise OracleProtocolError('Twist previews require a private settings scheme')
        interaction = TwistPreviewPicker(request)
    if any(op.get('op') == 'mirror_preview' for op in request.get('operations', [])):
        from .mirror_preview_input import MirrorPreviewPicker
        if client.settings_scheme is None:
            raise OracleProtocolError('Mirror previews require a private Rhino settings scheme')
        interaction = MirrorPreviewPicker(request)
    if any(op.get('op') == 'translation_preview' for op in request.get('operations',[])):
        from .translation_preview_input import TranslationPreviewPicker
        if client.settings_scheme is None:raise OracleProtocolError('translation previews require a private scheme')
        interaction=TranslationPreviewPicker(request)
    if any(op.get('op') == 'affine_preview' for op in request.get('operations',[])):
        from .affine_preview_input import AffinePreviewPicker
        if client.settings_scheme is None:raise OracleProtocolError('affine previews require a private scheme')
        interaction=AffinePreviewPicker(request)
    if any(op.get("op") == "transform_copy_command" for op in request.get("operations", [])):
        from .transform_copy_capture import validate_request
        if client.settings_scheme is None:
            raise OracleProtocolError("transform Copy probes require a private Rhino settings scheme")
        validate_request(request)
        if any('normal_target' in op or 'mouse_target' in op or op.get('mirror_target', {}).get('pick') in ('mouse', 'mouse-sub') for op in request['operations']):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    return interaction
