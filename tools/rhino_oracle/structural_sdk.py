"""Host-side validation and worker companions for owned structural SDK probes."""
from importlib import import_module
from pathlib import Path
import shutil

# The validator module is also the worker entry. Dependencies stay beside it in
# the isolated job directory so standalone Rhino imports resolve transitively.
PROBES = {
    'face_create_command': ('face_create_probe', ('join_probe','merge_edges_probe','face_create_input')),
    'planar_mesh_command': ('planar_mesh_probe', ('join_probe','merge_edges_probe')),
    'mesh_polyline_command': ('mesh_polyline_probe', ('join_probe','merge_edges_probe')),
    'mesh_patch_command': ('mesh_patch_probe', ('join_probe','merge_edges_probe')),
    'mesh_from_lines_command': ('mesh_from_lines_probe', ('join_probe','merge_edges_probe')),
    'polyline_on_mesh': ('polyline_on_mesh_probe', ('join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'surface_interpolate_command': ('surface_interpolate_command_probe', ('surface_short_path_probe','short_path_command_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'surface_interpolate': ('surface_interpolate_probe', ('surface_short_path_probe',)),
    'short_path_command': ('short_path_command_probe', ('surface_short_path_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'surface_short_path': ('surface_short_path_probe', ()),
    'surface_offset_command': ('surface_offset_command_probe', ('join_probe','merge_edges_probe','conic_getter_input','number_token','managed_mouse_callback','snap_environment')),
    'surface_offset_through_point': ('surface_offset_command_probe', ('join_probe','merge_edges_probe','conic_getter_input','number_token','managed_mouse_callback','snap_environment')),
    'surface_curve_offset': ('surface_curve_offset_probe', ('surface_offset_command_probe',)),
    'orient_curve_edge': ('orient_curve_edge_probe', ('join_probe','merge_edges_probe','number_token','conic_getter_input','managed_mouse_callback','snap_environment')),
    'curve_frame_boundary': ('curve_frame_boundary_probe', ()),
    'curve_array': ('curve_array_probe', ('join_probe','merge_edges_probe','number_token','conic_getter_input','managed_mouse_callback','snap_environment')),
    'array_surface': ('array_surface_probe', ('join_probe','merge_edges_probe','number_token','conic_getter_input','managed_mouse_callback','snap_environment')),
    'array_curve_surface': ('array_curve_surface_probe', ('join_probe','merge_edges_probe','number_token','conic_getter_input','managed_mouse_callback','snap_environment')),
    'splop_tangent': ('splop_tangent_probe', ('circle_around_probe','getter_recipe','curve_source_bounds','geometry_validity_probe','conic_getter_input','managed_mouse_callback','snap_environment','join_probe','merge_edges_probe')),
    'splop_finish': ('splop_finish_probe', ('splop_reference_probe','splop_probe','splop_grips_probe','grip_transform_probe','twist_command_probe','join_probe','merge_edges_probe')),
    'splop_guided': ('splop_guided_probe', ('splop_probe','twist_command_probe','join_probe','merge_edges_probe')),
    'splop_reference': ('splop_reference_probe', ('splop_probe','twist_command_probe','join_probe','merge_edges_probe')),
    'splop_grips': ('splop_grips_probe', ('grip_transform_probe','join_probe','merge_edges_probe')),
    'splop_command': ('splop_probe', ('twist_command_probe','join_probe','merge_edges_probe')),
    'morph_fit_readback': ('morph_fit_readback_probe', ('morph_fit_benchmark_probe','twist_command_probe')),
    'morph_fit_benchmark': ('morph_fit_benchmark_probe', ('twist_command_probe',)),
    'stretch_details': ('stretch_details_probe', ('twist_command_probe','grip_transform_probe')),
    'stretch_fit_sdk': ('stretch_fit_probe', ('twist_command_probe',)),
    'stretch_grips': ('stretch_grips_probe', ('stretch_geometry_probe','grip_transform_probe','taper_command_probe','twist_command_probe','number_token','join_probe','merge_edges_probe')),
    'stretch_geometry_command': ('stretch_geometry_probe', ('taper_command_probe','twist_command_probe','number_token','join_probe','merge_edges_probe')),
    'stretch_sdk': ('stretch_probe', ('join_probe','merge_edges_probe')),
    'stretch_command': ('stretch_probe', ('join_probe','merge_edges_probe')),
    'soft_edit_surface_closed_sequence': ('soft_edit_surface_closed_sequence_probe', ('soft_edit_surface_seams_probe','soft_edit_surface_shapes_probe','soft_edit_surface_probe','soft_edit_curve_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'soft_edit_surface_seams': ('soft_edit_surface_seams_probe', ('soft_edit_surface_shapes_probe','soft_edit_surface_probe','soft_edit_curve_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'soft_edit_surface_repeat_sdk': ('soft_edit_surface_repeat_sdk_probe', ()),
    'soft_edit_surface_sdk': ('soft_edit_surface_sdk_probe', ()),
    'soft_edit_surface_sequence': ('soft_edit_surface_sequence_probe', ('soft_edit_surface_probe','soft_edit_curve_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'soft_edit_surface_shapes': ('soft_edit_surface_shapes_probe', ('soft_edit_surface_probe','soft_edit_curve_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'soft_edit_surface_command': ('soft_edit_surface_probe', ('soft_edit_curve_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'soft_edit_curve_sequence': ('soft_edit_sequence_probe', ('soft_edit_curve_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'soft_edit_curve_lengths': ('soft_edit_lengths_probe', ()),
    'soft_edit_curve_command': ('soft_edit_curve_probe', ('insert_kink_probe','join_probe','merge_edges_probe')),
    'empty_undo_selection': ('empty_undo_selection_probe', ('join_probe','merge_edges_probe')),
    'insert_edit_point_history': ('insert_edit_point_history_probe', ('join_probe','merge_edges_probe')),
    'insert_edit_point_limits': ('insert_edit_point_limits_probe', ('insert_edit_point_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'insert_edit_point_command': ('insert_edit_point_probe', ('insert_kink_probe','join_probe','merge_edges_probe')),
    'grip_weight_nurbs': ('weight_nurbs_probe', ('grip_weight_probe','weight_dialog','delete_grips_probe','grip_navigation_probe','grip_transform_probe','join_probe','merge_edges_probe')),
    'grip_weight_lifecycle': ('weight_lifecycle_probe', ('weight_dialog','delete_grips_probe','grip_navigation_probe','grip_transform_probe','join_probe','merge_edges_probe')),
    'grip_weight': ('grip_weight_probe', ('weight_dialog','delete_grips_probe','grip_navigation_probe','grip_transform_probe','join_probe','merge_edges_probe')),
    'grip_delete_sequence': ('grip_delete_sequence_probe', ('delete_grips_probe','grip_navigation_probe','grip_transform_probe','join_probe','merge_edges_probe')),
    'delete_grips': ('delete_grips_probe', ('grip_navigation_probe','grip_transform_probe','join_probe','merge_edges_probe')),
    'grip_visibility': ('grip_visibility_probe', ('hide_pt_input','grip_navigation_probe','grip_transform_probe','join_probe','merge_edges_probe')),
    'select_uv_rows': ('sel_uv_probe', ('sel_uv_input', 'grip_navigation_probe', 'grip_transform_probe', 'join_probe', 'merge_edges_probe')),
    'grip_navigation': ('grip_navigation_probe', ('grip_transform_probe', 'join_probe', 'merge_edges_probe')),
    'curve_insert_control_point_geometry': ('insert_control_point_geometry_probe', ('join_probe', 'merge_edges_probe')),
    'surface_insert_control_point_geometry': ('insert_control_point_geometry_probe', ('join_probe', 'merge_edges_probe')),
    'insert_control_point_command': ('insert_control_point_probe', ('remove_knot_command_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'remove_control_point_composite_parameters': ('remove_control_point_composite_parameters_probe', ('remove_control_point_probe','remove_knot_command_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'remove_control_point_parameters': ('remove_control_point_parameters_probe', ('remove_control_point_probe','remove_knot_command_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'remove_control_point_command': ('remove_control_point_probe', ('remove_knot_command_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'remove_knot_smooth_periodic': ('remove_knot_smooth_periodic_probe', ('remove_knot_command_probe','insert_kink_probe','join_probe','merge_edges_probe')),
    'remove_knot_command': ('remove_knot_command_probe', ('insert_kink_probe','join_probe','merge_edges_probe')),
    'insert_knot_command': ('insert_knot_command_probe', ('insert_kink_probe','join_probe','merge_edges_probe')),
    'insert_kink_subcurve': ('insert_kink_subcurve_probe', ('insert_kink_probe','join_probe','merge_edges_probe','shrink_face_input','snap_environment')),
    'insert_kink_command': ('insert_kink_probe', ('join_probe','merge_edges_probe')),
    'polyline_closure': ('polyline_closure_probe', ('join_probe','merge_edges_probe')),
    'mark_foci_command': ('mark_foci_probe', ('join_probe','merge_edges_probe')),
    'reduce_polyline_command': ('reduce_polyline_probe', ('join_probe','merge_edges_probe')),
    'nested_rational_transfer': ('nested_rational_probe', ()),
    'delete_subcurve_command': ('delete_subcurve_probe', ('join_probe','merge_edges_probe')),
    'modify_radius_command': ('modify_radius_probe', ('join_probe','merge_edges_probe')),
    'simplify_curve_command': ('simplify_curve_probe', ('join_probe','merge_edges_probe')),
    'remap_cplane': ('remap_cplane_probe', ('grip_transform_probe','join_probe','merge_edges_probe')),
    'arc_command': ('arc_command_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'ellipse_command': ('ellipse_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'circle_around': ('circle_around_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'circle_deform': ('circle_around_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'circle_tangent': ('circle_around_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'arc_tangent': ('arc_tangent_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'arc_extension': ('arc_extension_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'catenary_command': ('catenary_command_probe', ('curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'spiral_layout': ('spiral_layout_probe', ()),
    'helix_command': ('spiral_command_probe', ('curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'spiral_command': ('spiral_command_probe', ('curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'rectangle': ('rectangle_probe', ('curve_primitive_probe','geometry_validity_probe','join_probe','merge_edges_probe')),
    'hyperbola_command': ('hyperbola_probe', ('curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'parabola_command': ('parabola_probe', ('curve_primitive_probe','join_probe','merge_edges_probe')),
    'conic_command': ('conic_probe', ('curve_primitive_probe','geometry_validity_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'surface_flow': ('surface_flow_probe', ('geometry_validity_probe','join_probe','merge_edges_probe','snap_environment','viewport_capture')),
    'flow_roadlike': ('flow_roadlike_probe', ('flow_line_probe','flow_probe','geometry_validity_probe','merge_edges_probe','join_probe')),
    'arc_length_batch': ('arc_length_batch_probe', ('geometry_validity_probe',)),
    'flow_line': ('flow_line_probe', ('flow_probe','geometry_validity_probe','merge_edges_probe','join_probe')),
    'flow_grips': ('flow_grips_probe', ('geometry_validity_probe','flow_probe','merge_edges_probe','join_probe')),
    'flow': ('flow_probe', ('geometry_validity_probe',)),
    'orient_surface_options': ('orient_surface_options_probe', ('geometry_validity_probe','orient_surface_probe','orient_surface_dialog')),
    'orient_surface_grips': ('orient_surface_grips_probe', ('geometry_validity_probe','orient_surface_probe')),
    'orient_surface': ('orient_surface_probe', ('geometry_validity_probe',)),
    'orient_points': ('orient_points_probe', ()),
    'orient_on_curve': ('orient_on_curve_probe', ('geometry_validity_probe','orient_on_curve_edge_input','merge_edges_probe')),
    'curvature_sampling': ('curvature_sampling_probe', ('geometry_validity_probe','curvature_graph_probe')),
    'extract_curvature_graph': ('extract_curvature_graph_probe', ('geometry_validity_probe',)),
    'curvature_graph': ('curvature_graph_probe', ('geometry_validity_probe',)),
    'curve_deviation': ('curve_deviation_probe', ('geometry_validity_probe',)),
    'point_deviation': ('point_deviation_probe', ('geometry_validity_probe',)),
    'sphere_fit_points': ('sphere_fit_points_probe', ('line_through_points_probe','plane_through_points_probe')),
    'line_through_points': ('line_through_points_probe', ('plane_through_points_probe',)),
    'plane_through_points': ('plane_through_points_probe', ()),
    'network_surface': ('network_surface_probe', ('curve_recipe',)),
    'surface_fillet': ('surface_fillet_probe', ()),
    'reparameterize_face_command': ('reparameterize_face_probe', ('join_probe', 'merge_edges_probe', 'surface_fillet_probe')),
    'reparameterize_block_command': ('reparameterize_block_probe', ('block_workflow_probe', 'conic_getter_input', 'managed_mouse_callback', 'snap_environment', 'join_probe', 'merge_edges_probe')),
    'rail_revolve': ('rail_revolve_probe', ()),
    'two_rail_sweep': ('two_rail_sweep_probe', ()),
    'divide_along_creases': ('divide_creases_probe', ('geometry_validity_probe',)),
    'cull_degenerate_mesh_faces': ('cull_degenerate_probe', ('geometry_validity_probe',)),
    'geometry_validity': ('geometry_validity_probe', ()),
    'merge_coplanar_faces': ('coplanar_faces_probe', ('coplanar_mesh_probe',)),
    'selected_coplanar_faces': ('selected_coplanar_faces_probe', ('coplanar_faces_probe', 'coplanar_mesh_probe')),
}


def validate_operations(request):
    for operation in request.get('operations', []):
        specification = PROBES.get(operation.get('op'))
        if specification is None:
            continue
        import_module('.'+specification[0], __package__).validate(operation)
        if operation['op'] == 'selected_coplanar_faces' and operation['source']['type'] != 'mesh_brep':
            raise ValueError('selected mesh native getter capture remains pending')


def stage_helpers(request, source_directory: Path, destination: Path):
    modules = set()
    for operation in request.get('operations', []):
        specification = PROBES.get(operation.get('op'))
        if specification is not None:
            modules.add(specification[0])
            modules.update(specification[1])
    for module in sorted(modules):
        name = module+'.py'
        shutil.copyfile(source_directory/name, destination/name)
