"""B-rep editing, camera/view and document capture policy."""
from .oracle_errors import OracleProtocolError


def prepare(client, request, interaction=None):
    if any(op.get('op') == 'mesh_trim_command' for op in request.get('operations', [])):
        from .mesh_trim_cases import ALL_CASES, validate
        if client.settings_scheme is None or request.get('iterations', 1) != 1:
            raise OracleProtocolError('MeshTrim inputs require one iteration in a private scheme')
        for operation in request['operations']:
            validate(operation)
        if any('trim_picks' in ALL_CASES[op['case']] for op in request['operations']):
            from .mesh_trim_picking import MeshTrimPicker
            interaction = MeshTrimPicker()

    if any(op.get("op") == "copy_options_command" for op in request.get("operations", [])):
        from .copy_options_probe import validate
        if client.settings_scheme is None:
            raise OracleProtocolError("Copy settings probes require a private Rhino settings scheme")
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("Copy settings probes require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "copy_options_command": validate(operation)
    if any(op.get("op") == "brep_remove_holes" for op in request.get("operations", [])):
        from .remove_holes_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("hole removal requires one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "brep_remove_holes": validate(operation)
    if any(op.get("op") == "brep_unjoin_edges" for op in request.get("operations", [])):
        from .unjoin_edges_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("edge separation requires one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "brep_unjoin_edges": validate(operation)
    if any(op.get("op") == "unjoin_edge_command" for op in request.get("operations", [])):
        from .unjoin_edge_command_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("edge separation commands require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "unjoin_edge_command": validate(operation)
        if any(op.get("pick") in ("mouse", "sequence") for op in request["operations"]):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get("op") == "extract_srf_command" for op in request.get("operations", [])):
        from .extract_srf_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("ExtractSrf commands require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "extract_srf_command": validate(operation)
        if any(op.get("op") == "extract_srf_command" and op.get("pick") == "sequence" for op in request["operations"]):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get("op") in ("shrink_trimmed_srf_command", "shrink_trimmed_srf_to_edge_command") for op in request.get("operations", [])):
        from .shrink_trimmed_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("shrink commands require one iteration")
        for operation in request["operations"]:
            if operation.get("op") in ("shrink_trimmed_srf_command", "shrink_trimmed_srf_to_edge_command"): validate(operation)
        if any(op.get("pick") == "sequence" for op in request["operations"]):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get("op") == "untrim_command" for op in request.get("operations", [])):
        from .untrim_component_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("Untrim commands require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "untrim_command": validate(operation)
        if any(op.get("pick") == "mouse" for op in request["operations"]):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get("op") in ("document_brep", "document_brep_import") for op in request.get("operations", [])):
        from .document_brep_probe import validate
        for operation in request["operations"]:
            if operation.get("op") in ("document_brep", "document_brep_import"): validate(operation, request.get("iterations", 1))
    if any(op.get("op") == "orientation_audit" for op in request.get("operations", [])):
        from .orientation_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("orientation audit requires one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "orientation_audit": validate(operation)
    if any(op.get("op") in ("untrim_all_command", "untrim_border_command") for op in request.get("operations", [])):
        from .untrim_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("untrim commands require one iteration")
        for operation in request["operations"]:
            if operation.get("op") in ("untrim_all_command", "untrim_border_command"): validate(operation)
    if any(op.get("op") == "untrim_holes_command" for op in request.get("operations", [])):
        from .untrim_holes_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("hole commands require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "untrim_holes_command": validate(operation)
        if any(op.get("pick") in ("mouse", "window") for op in request["operations"]):
            # The shared picker also accepts sequenced UnjoinEdge inputs
            # when a protocol request mixes these component commands.
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get("op") == "view_camera_probe" for op in request.get("operations", [])):
        from .view_camera_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("camera probes require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "view_camera_probe":
                validate(operation)
    if any(op.get("op") == "set_view_prompt_probe" for op in request.get("operations", [])):
        from .set_view_prompt_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("SetView prompt probes require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "set_view_prompt_probe":
                validate(operation)
    if any(op.get("op") == "named_view_policy_probe" for op in request.get("operations", [])):
        from .named_view_policy_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("named-view policy probes require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "named_view_policy_probe":
                validate(operation)
    if any(op.get("op") == "zoom_extents_probe" for op in request.get("operations", [])):
        from .zoom_extents_probe import validate
        if type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1:
            raise OracleProtocolError("Zoom fitting probes require one iteration")
        for operation in request["operations"]:
            if operation.get("op") == "zoom_extents_probe":
                validate(operation)
    if any(op.get("op") in ("synchronize_cplanes_probe", "copy_cplane_probe")
           for op in request.get("operations", [])):
        from .group_picking import IdlePicker
        interaction = IdlePicker()
    if any(op.get("op") == "view_camera_probe" and "mouse_drag" in op
           for op in request.get("operations", [])):
        from .camera_navigation import CameraNavigator
        interaction = CameraNavigator(request)
    if any(op.get("op") in ("area_centroid_command", "volume_centroid_command", "volume_command") for op in request.get("operations", [])):
        from .area_centroid_probe import validate
        if type(request.get("iterations",1)) is not int or request.get("iterations",1) != 1:
            raise OracleProtocolError("centroid commands require one iteration")
        for operation in request["operations"]:
            if operation.get("op") in ("area_centroid_command", "volume_centroid_command", "volume_command"):
                validate(operation, operation["op"].split("_")[0], operation["op"] != "volume_command")
    if any("open_confirmation" in op for op in request.get("operations", [])):
        from .volume_confirmation import VolumeConfirmation
        interaction = VolumeConfirmation(request)
    return interaction
