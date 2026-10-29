"""Source recipes, reference imports, fitting and transform capture policy."""
from .oracle_errors import OracleProtocolError


def validate_sources(client, request, interaction=None):
    if any(op.get('op') in ('mesh_boolean_sdk', 'mesh_boolean_command', 'mesh_outline_sdk', 'mesh_outline_command') for op in request.get('operations', [])):
        operations = request['operations']
        if (client.settings_scheme is None or type(request.get('iterations', 1)) is not int
                or request.get('iterations', 1) != 1 or not 1 <= len(operations) <= 64
                or len({op.get('op') for op in operations}) != 1
                or len({op['id'] for op in operations}) != len(operations)):
            raise OracleProtocolError('Mesh outline captures require an owned private scheme and bounded unique fixed inputs')
    if any(op.get('op') in ('mesh_weld_channels','mesh_weld_channels_followup','mesh_weld_channels_prefixes','mesh_unweld_channels','mesh_component_unweld_channels','mesh_component_unweld_commands','mesh_component_weld_commands','mesh_component_weld_followup','mesh_component_weld_holdout','component_seams_subd') for op in request.get('operations', [])):
        operations=request['operations']
        if (client.settings_scheme is None or type(request.get('iterations',1)) is not int
                or request.get('iterations',1)!=1 or not 1<=len(operations)<=64
                or len({op.get('op') for op in operations})!=1
                or len({op['id'] for op in operations})!=len(operations)):
            raise OracleProtocolError('Mesh welding captures require owned unique fixed inputs')
    if any(op.get('op') == 'mesh_channels_sdk' for op in request.get('operations', [])):
        operations=request['operations']
        if (client.settings_scheme is None or type(request.get('iterations',1)) is not int
                or request.get('iterations',1)!=1 or not 1<=len(operations)<=64
                or any(op.get('op')!='mesh_channels_sdk' for op in operations)
                or len({op['id'] for op in operations})!=len(operations)):
            raise OracleProtocolError('Mesh channel SDK captures require owned unique fixed inputs')
    if any(op.get('op') == 'control_polygon_command' for op in request.get('operations', [])):
        operations = request['operations']
        if (client.settings_scheme is None or type(request.get('iterations', 1)) is not int
                or request.get('iterations', 1) != 1 or not 1 <= len(operations) <= 64
                or any(op.get('op') != 'control_polygon_command' for op in operations)
                or len({op['id'] for op in operations}) != len(operations)):
            raise OracleProtocolError('Control-polygon commands require an owned private scheme and bounded unique fixed recipes')
    if any(op.get('op')in ('mesh_patch_command','mesh_from_lines_command','mesh_polyline_command','planar_mesh_command','face_create_command')for op in request.get('operations',[])):
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1:
            raise OracleProtocolError('Owned mesh commands require a private scheme and one iteration')
        if len(request['operations'])>32 or len({op['id']for op in request['operations']})!=len(request['operations']):
            raise OracleProtocolError('Owned mesh commands require bounded unique operation ids')
    if any(op.get('op') in ('plane_through_points','line_through_points','sphere_fit_points','curve_deviation') for op in request.get('operations', [])):
        if client.settings_scheme is None or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Owned fitting/deviation commands require a private scheme and one iteration')
    return interaction


def prepare(client, request, interaction=None):
    if any(op.get('op') == 'remap_cplane' for op in request.get('operations', [])):
        if client.settings_scheme is None or request.get('iterations', 1) != 1:
            raise OracleProtocolError('RemapCPlane requires a private scheme and one iteration')
        from .remap_cplane_probe import validate_capture
        for op in request['operations']:
            if op.get('op') == 'remap_cplane':
                validate_capture(op)
    if any(op.get('op') == 'fillet_edge_reference' for op in request.get('operations', [])):
        from .fillet_edge_probe import validate
        if client.settings_scheme is None or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Edge fillet references require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'fillet_edge_reference':
                validate(op)
    if any(op.get('op') in ('step_poles', 'step_regions') for op in request.get('operations', [])):
        from .step_poles_probe import validate
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('STEP fixture imports require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') in ('step_poles', 'step_regions'):
                validate(op)
    if any(op.get('op') == 'smlib_reference' for op in request.get('operations', [])):
        from .smlib_reference_probe import validate
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('SMLib references require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'smlib_reference':
                validate(op)
    if any(op.get('op')=='divide_preferences' for op in request.get('operations',[])):
        from .divide_preferences_probe import validate
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1:
            raise OracleProtocolError('Divide preferences require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op')=='divide_preferences':validate(op)
    if any(op.get('op')=='divide_command' for op in request.get('operations',[])):
        from .divide_probe import validate
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1:
            raise OracleProtocolError('Divide commands require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op')=='divide_command':validate(op)
    if any(op.get('op') == 'edge_analysis' for op in request.get('operations', [])):
        from .edge_analysis_probe import validate
        for op in request['operations']:
            if op.get('op') == 'edge_analysis':
                validate(op)
                if 'workflow' in op and (client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1):
                    raise OracleProtocolError('Edge-analysis workflows require a private scheme and one iteration')
    if any(op.get('op')=='contour_command' for op in request.get('operations',[])):
        from .contour_probe import validate
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
            raise OracleProtocolError('Contour probes require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op')=='contour_command':validate(op)
    if any(op.get('op') == 'iplane' for op in request.get('operations', [])):
        from .iplane_probe import validate
        for op in request['operations']:
            if op.get('op') == 'iplane': validate(op)
    if any(op.get('op')=='section_command' for op in request.get('operations',[])):
        from .section_probe import validate
        for op in request['operations']:
            if op.get('op')=='section_command':validate(op)
    if any(op.get('op') == 'block_workflow' for op in request.get('operations', [])):
        from .block_workflow_probe import validate
        if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise ValueError('block workflows require one iteration')
        for operation in request['operations']:
            if operation.get('op') == 'block_workflow':
                validate(operation)
    if any(op.get('op') == 'scale_nu' for op in request.get('operations', [])):
        from .scale_nu_probe import validate
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('ScaleNU probes require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'scale_nu':
                validate(op)
    if any(op.get('op') == 'mesh_edit_records' for op in request.get('operations', [])):
        from .mesh_edit_records_probe import validate
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Mesh record probes require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'mesh_edit_records':
                validate(op)
    for op in request.get('operations', []):
        if op.get('op') == 'silhouette_parallel':
            from .silhouette_probe import validate
            validate(op)
        elif op.get('op') == 'surface_path_extrusion':
            from .surface_path_extrusion_probe import validate
            validate(op)
        elif op.get('op') == 'surface_apex_extrusion':
            from .surface_apex_extrusion_probe import validate
            validate(op)
        elif op.get('op') == 'blend_edge':
            from .blend_edge_probe import validate
            validate(op)
        elif op.get('op') == 'chamfer_edge':
            from .chamfer_edge_probe import validate
            validate(op)
        elif op.get('op') == 'shell':
            from .shell_probe import validate
            validate(op)
        elif op.get('op') == 'surface_tapered_extrusion':
            from .surface_tapered_extrusion_probe import validate
            validate(op)
        elif op.get('op') == 'surface_extrusion':
            from .surface_extrusion_probe import validate
            validate(op)
        elif op.get('op') == 'tapered_extrusion':
            from .tapered_extrusion_probe import validate
            validate(op)
    if any(op.get('op') == 'grip_transform' for op in request.get('operations', [])):
        from .grip_transform_probe import validate
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
            raise OracleProtocolError('Grip transforms require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'grip_transform':
                validate(op)
    if any(op.get('op') in ('circle_fit_grips', 'circle_fit_grips_commands') for op in request.get('operations', [])):
        from .circle_fit_grips_probe import validate
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
            raise OracleProtocolError('Circle grip captures require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') in ('circle_fit_grips', 'circle_fit_grips_commands'):
                validate(op)
    if any(op.get('op') == 'circle_fit_diagnostics' for op in request.get('operations', [])):
        from .circle_fit_diagnostics import validate
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
            raise OracleProtocolError('Circle diagnostics require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'circle_fit_diagnostics':
                validate(op)
    if any(op.get('op') == 'circle_fit_selection' for op in request.get('operations', [])):
        from .circle_fit_selection_probe import validate
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
            raise OracleProtocolError('Circle selection captures require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'circle_fit_selection':
                validate(op)
    if any(op.get('op') == 'circle_fit_benchmark' for op in request.get('operations', [])):
        from .circle_fit_benchmark import validate
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
            raise OracleProtocolError('Circle benchmarks require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'circle_fit_benchmark':
                validate(op)
    if any(op.get('op') == 'circle_fit_points' for op in request.get('operations', [])):
        from .circle_fit_probe import validate
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
            raise OracleProtocolError('Circle fit commands require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'circle_fit_points':
                validate(op)
    for family, module in [('maelstrom_geometry_command', 'maelstrom_command_probe'),
                           ('maelstrom_options_command', 'maelstrom_options_probe'),
                           ('maelstrom_input_command', 'maelstrom_input_probe'),
                           ('maelstrom_circle_command', 'maelstrom_circle_probe'),
                           ('maelstrom_fit_points_command', 'maelstrom_fit_points_probe')]:
        if any(op.get('op') == family for op in request.get('operations', [])):
            from importlib import import_module
            validate = import_module('.'+module, __package__).validate
            if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
                raise OracleProtocolError('Maelstrom commands require a private scheme and one iteration')
            for op in request['operations']:
                if op.get('op') == family:
                    validate(op)
    if any(op.get('op') in ('maelstrom_points','maelstrom_command_points') for op in request.get('operations', [])):
        from .maelstrom_probe import validate
        for op in request['operations']:
            if op.get('op') in ('maelstrom_points','maelstrom_command_points'):
                validate(op)
            if op.get('op') == 'maelstrom_command_points' and (client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1):
                raise OracleProtocolError('Maelstrom commands require a private scheme and one iteration')
    if any(op.get('op') == 'taper_options_command' for op in request.get('operations', [])):
        from .taper_options_probe import validate
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Taper preferences require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'taper_options_command':
                validate(op)
    if any(op.get('op') == 'taper_geometry_command' for op in request.get('operations', [])):
        from .taper_command_probe import validate
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Taper geometry commands require a private scheme and one iteration')
        for op in request['operations']:
            if op.get('op') == 'taper_geometry_command':
                validate(op)
    if any(op.get('op') in ('taper_points', 'taper_command_points') for op in request.get('operations', [])):
        from .taper_probe import validate
        for op in request['operations']:
            if op.get('op') in ('taper_points', 'taper_command_points'):
                validate(op)
            if op.get('op') == 'taper_command_points':
                if client.settings_scheme is None:
                    raise OracleProtocolError('Taper commands require a private Rhino settings scheme')
                if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                    raise OracleProtocolError('Taper commands require one iteration')
    for family in ("twist_points", "twist_command"):
        cases=[op for op in request.get("operations",[]) if op.get("op")==family]
        if cases:
            from . import twist_probe, twist_command_probe
            if family=="twist_command" and client.settings_scheme is None:
                raise OracleProtocolError("Twist commands require a private Rhino settings scheme")
            if family=="twist_command" and (type(request.get("iterations",1)) is not int or request.get("iterations",1)!=1):
                raise OracleProtocolError("Twist commands require one iteration")
            for op in cases:
                (twist_probe if family=="twist_points" else twist_command_probe).validate(op)
    if any(op.get('op') in ('bend_points', 'bend_command_points') for op in request.get('operations', [])):
        from .bend_probe import validate
        for op in request['operations']:
            if op.get('op') in ('bend_points', 'bend_command_points'):
                validate(op)
            if op.get('op') == 'bend_command_points':
                if client.settings_scheme is None:
                    raise OracleProtocolError('Bend commands require a private Rhino settings scheme')
                if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
                    raise OracleProtocolError('Bend commands require one iteration')
    if any(op.get('op') == 'twist_options_command' for op in request.get('operations', [])):
        from .twist_options_probe import validate
        if client.settings_scheme is None:
            raise OracleProtocolError('Twist preferences require a private Rhino settings scheme')
        if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Twist preferences require one iteration')
        for op in request['operations']:
            if op.get('op') == 'twist_options_command':
                validate(op)
    if any(op.get('op') == 'bend_geometry_command' for op in request.get('operations', [])):
        from .bend_command_probe import validate
        if client.settings_scheme is None:
            raise OracleProtocolError('Bend geometry commands require a private Rhino settings scheme')
        if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Bend geometry commands require one iteration')
        for op in request['operations']:
            if op.get('op') == 'bend_geometry_command':
                validate(op)
    if any(op.get('op') == 'bend_options_command' for op in request.get('operations', [])):
        from .bend_options_probe import validate
        if client.settings_scheme is None:
            raise OracleProtocolError('Bend preferences require a private Rhino settings scheme')
        if type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Bend preferences require one iteration')
        for op in request['operations']:
            if op.get('op') == 'bend_options_command':
                validate(op)
    return interaction
