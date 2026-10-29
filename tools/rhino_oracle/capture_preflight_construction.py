"""Component selection, drafting and SubD construction capture policy."""
from .oracle_errors import OracleProtocolError


def prepare(client, request, interaction=None):
    interaction = None
    if any(op.get('op') == 'extract_points_command' for op in request.get('operations', [])):
        from .extract_points_cases import ALL_CASES, validate
        if client.settings_scheme is None or request.get('iterations',1) != 1 or any(op.get('op') != 'extract_points_command' for op in request['operations']):
            raise OracleProtocolError('ExtractPt inputs require a dedicated private one-iteration request')
        for op in request['operations']:validate(op)
        if any(ALL_CASES[op['case']].get('component_getter') or ALL_CASES[op['case']].get('edge_ranges') for op in request['operations']):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get('op')=='nonplanar_quad_sample'for op in request.get('operations',[])):
        from .nonplanar_sample_cases import CASES
        operations=request['operations']
        if client.settings_scheme is None or request.get('iterations',1)!=1 or any(op.get('op')!='nonplanar_quad_sample'for op in operations):
            raise OracleProtocolError('Face threshold getters require an owned dedicated one-iteration request')
        from .catenary_point_picker import CatenaryPointPicker
        interaction=CatenaryPointPicker(dict(request,operations=[dict(op,actions=CASES[op['case']].get('actions',[]))for op in operations]))
    if any(op.get('op') == 'control_polygon_command' for op in request.get('operations', [])):
        from .control_polygon_cases import PICKS
        if any(op.get('case') in PICKS for op in request['operations']):
            from .component_picking import ComponentPicker
            interaction = ComponentPicker()
    if any(op.get('op') == 'insert_point_mesh_sdk' for op in request.get('operations', [])):
        if client.settings_scheme is None or request.get('iterations',1)!=1 or any(op.get('op')!='insert_point_mesh_sdk' for op in request['operations']):
            raise OracleProtocolError('Insertion meshing SDK calibration requires a dedicated private workflow')
    if any(op.get('op') == 'insert_point_command' for op in request.get('operations', [])):
        from .insert_point_cases import ALL_CASES
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1 or any(op.get('op')!='insert_point_command' for op in operations):
            raise OracleProtocolError('InsertPoint requires a dedicated private workflow')
        if not 1<=len(operations)<=128 or len({op['id']for op in operations})!=len(operations):raise OracleProtocolError('InsertPoint requires 1 through 128 unique recipes')
        if any(ALL_CASES[op['case']].get('live')for op in operations):
            from .catenary_point_picker import CatenaryPointPicker
            interaction=CatenaryPointPicker(dict(request,operations=[dict(op,actions=[dict(click=True)if isinstance(a,dict)else a for a in ALL_CASES[op['case']]['actions']])for op in operations]))
    if any(op.get('op') == 'subd_spin_edge' for op in request.get('operations', [])):
        from .subd_spin_edge_cases import ALL_CASES
        operations = request['operations']
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1 or any(op.get('op') != 'subd_spin_edge' for op in operations):
            raise OracleProtocolError('SubD edge spins require a dedicated private workflow')
        if not 1 <= len(operations) <= 128 or len({op['id'] for op in operations}) != len(operations):
            raise OracleProtocolError('SubD edge spins require 1 through 128 unique recipes')
        if any(ALL_CASES[op['case']].get('live') for op in operations):
            from .catenary_point_picker import CatenaryPointPicker
            interaction = CatenaryPointPicker(dict(request, operations=[dict(op, actions=[
                dict(click=True) if isinstance(a, dict) else a
                for a in ALL_CASES[op['case']]['actions']]) for op in operations]))
    if any(op.get('op') == 'soft_crease_command' for op in request.get('operations', [])):
        from .soft_crease_cases import all_cases
        cases = all_cases()
        operations = request['operations']
        if client.settings_scheme is None or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1:
            raise OracleProtocolError('Soft crease workflows require a private scheme and one iteration')
        if any(op.get('op') != 'soft_crease_command' for op in operations):
            raise OracleProtocolError('Soft crease workflows require a dedicated request')
        names = [op['id'] for op in operations]
        if not 1 <= len(names) <= 128 or len(set(names)) != len(names):
            raise OracleProtocolError('Soft crease workflows require 1 through 128 unique recipes')
        if any(cases[op['case']].get('live') for op in operations):
            from .catenary_point_picker import CatenaryPointPicker
            # The fixed native recipe calculates each vertex target. The
            # host picker consumes only allowlisted action markers.
            click_request = dict(request, operations=[dict(op, actions=[
                dict(click=True) if isinstance(action, dict) else action
                for action in cases[op['case']]['actions']]) for op in operations])
            interaction = CatenaryPointPicker(click_request)
    if any(op.get('op')=='face_create_command' and op.get('case') in ('edge_single','cancel_partial','append_cancel_partial','multiple_cancel_partial','append_after_face_cancel_partial') for op in request.get('operations', [])):
        from .face_create_probe import control_request
        from .group_picking import IdlePicker
        if request != control_request():
            raise OracleProtocolError('3DFace physical inputs require the exact dedicated control sequence')
        interaction = IdlePicker()
    if any(op.get('op')=='face_create_command' and op.get('case','').startswith(('chain_','edge_multiple','edge_triangles','edge_ngons','edge_collinear','edge_flip','edge_overlap','face_free_','edge_undo_')) for op in request.get('operations', [])):
        from .face_draft_cases import request as draft_request,extra_request,corner_request,corner_control_request,corner_finish_request
        from .group_picking import IdlePicker
        if request not in (draft_request(),extra_request(),corner_request(),corner_control_request(),corner_finish_request()):
            raise OracleProtocolError('3DFace chain inputs require the exact dedicated draft sequence')
        interaction = IdlePicker()
    if any(op.get('op')=='face_create_command' and op.get('case','').startswith(('subd_draft_','subd_interp_draft_','subd_interp_order_')) for op in request.get('operations',[])):
        from .face_create_probe import subd_draft_request,subd_interp_draft_request,subd_interp_order_request
        from .group_picking import IdlePicker
        if request not in (subd_draft_request(),subd_interp_draft_request(),subd_interp_order_request()):
            raise OracleProtocolError('SubD face inputs require the exact dedicated draft sequence')
        interaction = IdlePicker()
    if any(op.get('op')=='reparameterize_block_command' for op in request.get('operations',[])):
        from .reparameterize_block_probe import validate
        from .reparameterize_block_picker import BlockReferencePicker
        operations=request['operations']
        if (client.settings_scheme is None or type(request.get('iterations',1))is not int
                or request.get('iterations',1)!=1 or not 1<=len(operations)<=64
                or any(op.get('op')!='reparameterize_block_command' for op in operations)):
            raise OracleProtocolError('Block reparameterization requires a private dedicated one-iteration capture')
        for op in operations:validate(op)
        interaction=BlockReferencePicker(request)
    if any(op.get('op')=='arc_command'for op in request.get('operations',[])):
        from .arc_command_probe import validate
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='arc_command'for op in operations):raise OracleProtocolError('Arc construction requires a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('Arc construction requires 1 through 64 unique cases')
        for op in operations:validate(op)
        if any(isinstance(a,dict)and'click'in a for op in operations for a in op['actions']):
            from .arc_point_picker import ArcPointPicker
            interaction=ArcPointPicker(request)
    if any(op.get('op')=='arc_tangent'for op in request.get('operations',[])):
        from .arc_tangent_probe import validate
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='arc_tangent'for op in operations):raise OracleProtocolError('Arc Tangent requires a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('Arc Tangent requires 1 through 64 unique cases')
        for op in operations:validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in op['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    if any(op.get('op')=='arc_extension'for op in request.get('operations',[])):
        from .arc_extension_probe import validate
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='arc_extension'for op in operations):raise OracleProtocolError('Arc Extension requires a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('Arc Extension requires 1 through 64 unique cases')
        for op in operations:validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in op['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    if any(op.get('op')in('surface_offset_command','surface_offset_through_point')for op in request.get('operations',[])):
        from .surface_offset_command_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('Surface offset command captures require a private settings scheme')
        validate_request(request)
    if any(op.get('op')=='surface_curve_offset'for op in request.get('operations',[])):
        from .surface_curve_offset_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('Surface offset SDK captures require a private settings scheme')
        validate_request(request)
    if any(op.get('op')=='short_path_command'for op in request.get('operations',[])):
        from .short_path_command_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('ShortPath command captures require a private settings scheme')
        validate_request(request)
    if any(op.get('op')=='polyline_on_mesh'for op in request.get('operations',[])):
        from .polyline_on_mesh_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('Mesh polyline captures require a private settings scheme')
        validate_request(request)
    if any(op.get('op')=='surface_interpolate_command'for op in request.get('operations',[])):
        from .surface_interpolate_command_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('Surface interpolation command captures require a private settings scheme')
        validate_request(request)
        if any(isinstance(a,dict)and'click'in a for op in request['operations']for a in op['actions']):
            from .catenary_point_picker import CatenaryPointPicker
            interaction=CatenaryPointPicker(request)
    if any(op.get('op')=='surface_interpolate'for op in request.get('operations',[])):
        from .surface_interpolate_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('Surface interpolation captures require a private settings scheme')
        validate_request(request)
    if any(op.get('op')=='surface_short_path'for op in request.get('operations',[])):
        from .surface_short_path_probe import validate_request
        if client.settings_scheme is None:raise OracleProtocolError('ShortPath SDK captures require a private settings scheme')
        validate_request(request)
    if any(op.get('op')=='orient_curve_edge'for op in request.get('operations',[])):
        from .orient_curve_edge_probe import validate
        from .arc_point_picker import ArcPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='orient_curve_edge'for op in operations):raise OracleProtocolError('OrientCrvToEdge requires a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('OrientCrvToEdge requires 1 through 64 unique cases')
        for op in operations:validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in op['actions']])for op in operations])
        interaction=ArcPointPicker(click_request)
    if any(op.get('op')=='curve_array'for op in request.get('operations',[])):
        from .curve_array_probe import validate
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='curve_array'for op in operations):raise OracleProtocolError('ArrayCrv requires a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('ArrayCrv requires 1 through 64 unique cases')
        for op in operations:validate(op)
        if any(op['pick_point']is not None for op in operations):
            from .catenary_point_picker import CatenaryPointPicker
            rows=[]
            for op in operations:
                index=(op['source_count']+1 if op['postselect']else 0)+(2 if op['base_point']is not None else 0)+int(op['subcurve']is not None)
                actions=[None]*(index+1)
                if op['pick_point']is not None:actions[index]=dict(click=op['pick_point'])
                rows.append(dict(op,actions=actions))
            interaction=CatenaryPointPicker(dict(request,operations=rows))
    if any(op.get('op') == 'array_surface' for op in request.get('operations', [])):
        from .array_surface_probe import validate
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='array_surface'for op in operations):raise OracleProtocolError('ArraySrf requires a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('ArraySrf requires 1 through 64 unique cases')
        for op in operations:validate(op)
    if any(op.get('op') == 'array_curve_surface' for op in request.get('operations', [])):
        from .array_curve_surface_probe import validate
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='array_curve_surface'for op in operations):
            raise OracleProtocolError('ArrayCrvOnSrf requires a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('ArrayCrvOnSrf requires 1 through 64 unique cases')
        for op in operations:validate(op)
        if any(op['pick_point']is not None for op in operations):
            from .catenary_point_picker import CatenaryPointPicker
            clicks=[]
            for op in operations:
                index=(op['source_count']+1 if op['postselect']else 0)+1+int(op['subcurve']is not None)
                actions=[None]*(index+1)
                if op['pick_point']is not None:actions[index]=dict(click=op['pick_point'])
                clicks.append(dict(op,actions=actions))
            interaction=CatenaryPointPicker(dict(request,operations=clicks))
    if any(op.get('op') == 'splop_tangent' for op in request.get('operations', [])):
        from .splop_tangent_probe import validate
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1))is not int or request.get('iterations',1)!=1 or any(op.get('op')!='splop_tangent'for op in operations):
            raise OracleProtocolError('Splop tangent reference getters require a private dedicated one-iteration capture')
        names=[op.get('id')for op in operations]
        if not 1<=len(names)<=64 or any(not isinstance(n,str)for n in names)or len(set(names))!=len(names):raise OracleProtocolError('Splop tangent reference getters require 1 through 64 unique cases')
        for op in operations:validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in op['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    if any(op.get('op') == 'subd_cone_references' for op in request.get('operations', [])):
        from .subd_cone_references_probe import validate, SCENES
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1 or any(op.get('op')!='subd_cone_references'for op in operations):
            raise OracleProtocolError('SubDCone reference input requires a private dedicated one-iteration capture')
        for op in operations: validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in SCENES[op['case']]['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    if any(op.get('op') == 'subd_truncated_cone_references' for op in request.get('operations', [])):
        from .subd_truncated_cone_references_probe import validate, SCENES
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1 or any(op.get('op')!='subd_truncated_cone_references'for op in operations):
            raise OracleProtocolError('SubDTruncatedCone reference input requires a private dedicated one-iteration capture')
        for op in operations: validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in SCENES[op['case']]['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    if any(op.get('op') == 'subd_sphere_references' for op in request.get('operations', [])):
        from .subd_sphere_references_probe import validate, SCENES
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1 or any(op.get('op')!='subd_sphere_references'for op in operations):
            raise OracleProtocolError('SubDSphere reference input requires a private dedicated one-iteration capture')
        for op in operations: validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in SCENES[op['case']]['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    if any(op.get('op') == 'subd_torus_references' for op in request.get('operations', [])):
        from .subd_torus_references_probe import validate, SCENES
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1 or any(op.get('op')!='subd_torus_references'for op in operations):
            raise OracleProtocolError('SubDTorus reference input requires a private dedicated one-iteration capture')
        for op in operations: validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in SCENES[op['case']]['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    if any(op.get('op') == 'subd_cylinder_references' for op in request.get('operations', [])):
        from .subd_cylinder_references_probe import validate, SCENES, EXTRA_CASES
        from .catenary_point_picker import CatenaryPointPicker
        operations=request['operations']
        if client.settings_scheme is None or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1 or any(op.get('op')!='subd_cylinder_references'for op in operations):
            raise OracleProtocolError('SubDCylinder reference input requires a private dedicated one-iteration capture')
        for op in operations: validate(op)
        click_request=dict(request,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a and'pick'in a else a for a in (SCENES|EXTRA_CASES)[op['case']]['actions']])for op in operations])
        interaction=CatenaryPointPicker(click_request)
    return interaction
