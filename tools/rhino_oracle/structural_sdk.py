"""Host-side validation and worker companions for owned structural SDK probes."""
from importlib import import_module
from pathlib import Path
import shutil

# The validator module is also the worker entry. Dependencies stay beside it in
# the isolated job directory so standalone Rhino imports resolve transitively.
PROBES = {
    'arc_blend_command': ('arc_blend_command_probe', ('arc_blend_command','arc_blend_source_cases','arc_blend_subcurve_cases','arc_blend_measure','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'arc_blend_sdk': ('arc_blend_sdk', ('merge_edges_probe','arc_blend_roundoff','arc_blend_search_rules')),
    'line_pick_context': ('line_pick_context_probe', ('line_pick_context','line_tangent_rays','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_tangent_mouse': ('line_tangent_rays_probe', ('line_tangent_rays','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_tangent_rays': ('line_tangent_rays_probe', ('line_tangent_rays','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'tangent_zero_sdk': ('tangent_zero_sdk', ('tangent_zero_trial','merge_edges_probe')),
    'line_tangent_arcs_sdk': ('line_tangent_arcs_sdk', ('line_tangent_arcs_probe','line_tangent_arcs','line_tangent_arcs_ends','line_tangent_arcs_controls','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_tangent_arcs': ('line_tangent_arcs_probe', ('line_tangent_arcs','line_tangent_arcs_ends','line_tangent_arcs_controls','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_tangent': ('line_tangent_probe', ('line_tangent_cases','line_tangent_holdouts','line_tangent_final','line_tangent_completion','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_perpendicular': ('line_perpendicular_probe', ('line_perpendicular_cases','line_perpendicular_holdouts','line_perpendicular_final','line_perpendicular_completion','line_perpendicular_boundaries','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_normal': ('line_normal_probe', ('line_normal_cases','line_normal_holdouts','line_normal_final','line_normal_brep','move_normal_probe','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_reference': ('line_reference_probe', ('line_reference_cases','line_reference_holdouts','line_reference_completion','line_reference_final','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'line_construction': ('line_construction_probe', ('line_construction_cases','line_construction_holdouts','line_construction_precision','line_construction_completion','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'lines_command': ('lines_probe', ('lines_cases','lines_precision_cases','lines_finish_cases','lines_boundary_cases','lines_macro_cases','line_scene_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'command_file': ('command_file_probe', ('command_file_cases','command_file_holdouts','command_file_extras','command_file_polyline','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'point_grid_input': ('point_grid_input_probe', ('point_grid_input_cases','point_grid_input_holdouts','point_grid_input_prompts','conic_getter_input','managed_mouse_callback','snap_environment','number_token','join_probe','merge_edges_probe')),
    'srf_pt_command': ('srf_pt_probe', ('srf_pt_cases','srf_pt_holdouts','srf_pt_precision','srf_pt_admission','srf_pt_singular','srf_pt_origin','srf_pt_followup_cases','srf_pt_diagnostics','conic_getter_input','managed_mouse_callback','snap_environment','number_token','join_probe','merge_edges_probe')),
    'plane_surface_command': ('plane_surface_probe', ('plane_surface_cases','plane_surface_holdouts','plane_surface_guide_cases','plane_finish_cases','plane_finish_workflows','plane_finish_pointer_cases','plane_finish_pointer','plane_completion_cases','conic_getter_input','managed_mouse_callback','snap_environment','number_token','join_probe','merge_edges_probe')),
    'surface_interpolate_plane': ('surface_interpolate_plane_probe', ('surface_interpolate_plane_cases','surface_interpolate_plane_input','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment','number_token')),
    'curve_two_views': ('curve_two_views_probe', ('curve_two_views_cases','curve_two_views_fit_probe','curve_two_views_surface_probe','planar_regions_cases','join_probe','merge_edges_probe')),
    'curve_boolean': ('curve_boolean_probe', ('curve_boolean_cases','curve_boolean_subcurve_cases','curve_boolean_edge_input','owned_brep_command','planar_regions_cases','join_probe','merge_edges_probe')),
    'planar_chains': ('planar_chains_probe', ('planar_chains_cases','planar_regions_probe','planar_regions_cases','join_probe','merge_edges_probe')),
    'planar_regions': ('planar_regions_probe', ('planar_regions_cases','planar_crossings_cases','planar_crossings_probe','join_probe','merge_edges_probe')),
    'extrusion_splitting': ('extrusion_splitting_probe', ('extrusion_splitting_cases','extrude_apex_subd_cases','revolve_subd_cases','revolve_splitting_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'apex_plane_sdk': ('apex_plane_sdk_probe', ('apex_splitting_cases','extrude_apex_subd_cases','revolve_subd_cases','revolve_splitting_cases')),
    'apex_splitting': ('apex_splitting_probe', ('apex_splitting_cases','extrude_apex_subd_cases','revolve_subd_cases','revolve_splitting_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'extrude_apex_subd_sdk': ('extrude_apex_subd_sdk_probe', ('extrude_apex_subd_cases','revolve_subd_cases','revolve_splitting_cases','subd_box_probe')),
    'extrude_apex_subd': ('extrude_apex_subd_probe', ('extrude_apex_subd_cases','revolve_subd_cases','revolve_splitting_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'extrude_curve_subd_sdk': ('extrude_curve_subd_sdk_probe', ('extrude_curve_subd_cases','revolve_subd_cases','revolve_splitting_cases','subd_box_probe')),
    'extrude_curve_subd': ('extrude_curve_subd_probe', ('extrude_curve_subd_cases','revolve_subd_cases','revolve_splitting_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'revolve_subd_profile': ('revolve_subd_profile_probe', ('revolve_subd_cases','revolve_splitting_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'revolve_subd': ('revolve_subd_probe', ('revolve_subd_cases','revolve_splitting_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'revolve_splitting': ('revolve_splitting_probe', ('revolve_splitting_cases','join_probe','merge_edges_probe')),
    'revolve_domain_sampling': ('revolve_domain_sampling_probe', ('revolve_parameterization_probe','revolve_deformable_cases','join_probe','merge_edges_probe')),
    'revolve_parameterization': ('revolve_parameterization_probe', ('revolve_deformable_cases','join_probe','merge_edges_probe')),
    'revolve_deformable': ('revolve_deformable_probe', ('revolve_deformable_cases','join_probe','merge_edges_probe')),
    'short_span_extrusions': ('short_span_extrusions_probe', ('short_span_extrusions_cases','join_probe','merge_edges_probe')),
    'use_extrusions_creators': ('use_extrusions_creators_probe', ('use_extrusions_creators_cases','creator_extrusions_probe','join_probe','merge_edges_probe')),
    'use_extrusions': ('use_extrusions_probe', ('use_extrusions_cases','join_probe','merge_edges_probe')),
    'convert_extrusion': ('convert_extrusion_probe', ('convert_extrusion_cases','join_probe','merge_edges_probe')),
    'type_selection': ('type_selection_probe', ('type_selection_cases','type_selection_scene','join_probe','merge_edges_probe')),
    'edge_analysis_lifetime': ('edge_analysis_lifetime_probe', ('edge_analysis_lifetime_cases','edge_analysis_mixed_probe','edge_analysis_mixed_cases','subd_edge_analysis_probe','subd_edge_analysis_cases','owned_panel','edge_zoom_input','subd_direction_probe','subd_direction_cases','subd_volume_cases','subd_area_cases','component_length_cases','subd_to_nurbs_probe','subd_box_probe','subd_join_probe','subd_join_cases','edge_loop_cases','join_probe','merge_edges_probe')),
    'edge_analysis_mixed': ('edge_analysis_mixed_probe', ('edge_analysis_mixed_cases','subd_edge_analysis_probe','subd_edge_analysis_cases','owned_panel','edge_zoom_input','subd_direction_probe','subd_direction_cases','subd_volume_cases','subd_area_cases','component_length_cases','subd_to_nurbs_probe','subd_box_probe','subd_join_probe','subd_join_cases','edge_loop_cases','join_probe','merge_edges_probe')),
    'subd_edge_analysis': ('subd_edge_analysis_probe', ('subd_edge_analysis_cases','owned_panel','edge_zoom_input','subd_direction_probe','subd_direction_cases','subd_volume_cases','subd_area_cases','component_length_cases','subd_to_nurbs_probe','subd_box_probe','subd_join_probe','subd_join_cases','edge_loop_cases','join_probe','merge_edges_probe')),
    'subd_direction': ('subd_direction_probe', ('subd_direction_cases','subd_volume_cases','subd_area_cases','component_length_cases','subd_to_nurbs_probe','subd_box_probe','subd_join_probe','subd_join_cases','edge_loop_cases','join_probe','merge_edges_probe')),
    'subd_volume': ('subd_volume_probe', ('subd_volume_cases','component_length_cases','subd_to_nurbs_probe','subd_box_probe','subd_join_probe','subd_join_cases','edge_loop_cases','join_probe','merge_edges_probe')),
    'subd_area': ('subd_area_probe', ('subd_to_nurbs_probe','subd_box_probe','subd_area_cases','component_length_cases','subd_join_probe','subd_join_cases','edge_loop_cases','join_probe','merge_edges_probe')),
    'component_length': ('component_length_probe', ('component_length_cases','subd_join_probe','subd_join_cases','edge_loop_cases','merge_edges_probe','shrink_face_input','chain_selection_driver')),
    'length_chain': ('length_chain_probe', ('length_chain_cases','chain_selection_probe','chain_selection_cases','chain_selection_driver','shrink_face_input','merge_edges_probe')),
    'chain_selection': ('chain_selection_probe', ('chain_selection_cases','chain_selection_driver','shrink_face_input','merge_edges_probe')),
    'component_pattern_workflow': ('component_pattern_probe', ('component_pattern_cases','edge_ring_cases','edge_loop_workflow_probe','edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases','shrink_face_input','merge_edges_probe')),
    'face_loop_workflow': ('face_loop_probe', ('face_loop_cases','edge_ring_cases','edge_loop_workflow_probe','edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases','shrink_face_input','merge_edges_probe')),
    'edge_ring_workflow': ('edge_ring_probe', ('edge_ring_cases','edge_loop_workflow_probe','edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases','shrink_face_input','merge_edges_probe')),
    'edge_loop_workflow': ('edge_loop_workflow_probe', ('edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases','shrink_face_input','merge_edges_probe')),
    'edge_loop_gesture': ('edge_loop_gesture_cases', ('edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases')),
    'edge_loop_filtered_gesture': ('edge_loop_gesture_cases', ('edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases')),
    'edge_loop_subobject_gesture': ('edge_loop_gesture_cases', ('edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases')),
    'edge_loop_modified_gesture': ('edge_loop_gesture_cases', ('edge_loop_probe','edge_loop_cases','subd_join_probe','subd_join_cases')),
    'edge_loop_command': ('edge_loop_probe', ('edge_loop_cases','subd_join_probe','subd_join_cases','shrink_face_input','merge_edges_probe')),
    'nurbs_edge_command': ('nurbs_edge_command_probe', ('subd_packing_cases','subd_g1x_probe','subd_continuity_probe','subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_nurbs_packs': ('subd_nurbs_packs_probe', ('join_probe','merge_edges_probe')),
    'subd_pack_command': ('subd_pack_command_probe', ('subd_packing_cases','subd_g1x_probe','subd_continuity_probe','subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_soft_nurbs': ('subd_soft_nurbs_probe', ('subd_soft_nurbs_cases','subd_packing_cases','subd_g1x_probe','subd_continuity_probe','subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_nurbs_command': ('subd_nurbs_command_probe', ('subd_nurbs_command_cases','subd_packing_cases','subd_g1x_probe','subd_continuity_probe','subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_packing': ('subd_packing_probe', ('subd_packing_cases','subd_packing_holdouts','subd_g1x_probe','subd_continuity_probe','subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'subd_g1xx': ('subd_g1xx_probe', ('subd_g1x_probe','subd_continuity_probe','subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'subd_g1x': ('subd_g1x_probe', ('subd_continuity_probe','subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'subd_continuity': ('subd_continuity_probe', ('subd_g0_probe','subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'subd_to_nurbs_g0': ('subd_g0_probe', ('subd_to_nurbs_probe','subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'subd_to_nurbs_sdk': ('subd_to_nurbs_probe', ('subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'inset_closed_network': ('inset_closed_network_probe', ('inset_fan_basis_probe','inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_closed_seams': ('inset_closed_seams_probe', ('inset_fan_basis_probe','inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_closed_network_faces': ('inset_closed_network_probe', ('inset_fan_basis_probe','inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_fan_commands': ('inset_fan_commands_probe', ('inset_fan_interior_probe','inset_fan_basis_probe','inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_fan_interior': ('inset_fan_interior_probe', ('inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_fan_refinement': ('inset_fan_basis_probe', ('inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_fan_basis': ('inset_fan_basis_probe', ('inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_fan_surface_fit': ('inset_fan_surface_probe', ('inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_fan_fit': ('inset_fan_fit_probe', ('inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_collinear_sdk': ('inset_collinear_probe', ('inset_continuity_probe','inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_continuity_limits': ('inset_continuity_probe', ('inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_continuity_sdk': ('inset_continuity_probe', ('inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_continuity_command': ('inset_continuity_probe', ('inset_multiedge_probe','inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_multiedge_sdk': ('inset_multiedge_probe', ('inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_multiedge_command': ('inset_multiedge_probe', ('inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_cusp_sdk': ('inset_cusp_probe', ('inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_cusp_command': ('inset_cusp_probe', ('inset_outputs_probe','inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_output_limits': ('inset_outputs_probe', ('inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_output_modes': ('inset_outputs_probe', ('inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_output_preferences': ('inset_outputs_probe', ('inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_loose_controls': ('inset_loose_controls', ('inset_planar_curves','inset_surface_probe','merge_edges_probe')),
    'inset_loose_command': ('inset_loose_command', ('inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_loose_preferences': ('inset_loose_command', ('inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_planar_curves_command': ('inset_planar_curves_command', ('inset_planar_curves','inset_surface_probe','inset_surface_command','join_probe','merge_edges_probe')),
    'inset_planar_curves': ('inset_planar_curves', ('inset_surface_probe','merge_edges_probe')),
    'inset_surface_controls': ('inset_surface_controls', ('inset_surface_probe','merge_edges_probe')),
    'inset_surface_ready': ('inset_surface_command', ('inset_surface_probe','join_probe','merge_edges_probe')),
    'inset_surface_command': ('inset_surface_command', ('inset_surface_probe','join_probe','merge_edges_probe')),
    'inset_surface_sdk': ('inset_surface_probe', ('merge_edges_probe',)),
    'inset_preferences': ('inset_preferences',('inset_probe','inset_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'inset_extra_faces': ('inset_extra_probe',('inset_probe','inset_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'inset_mesh_faces': ('inset_mesh_probe',('inset_probe','inset_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'inset_mesh_confirmed': ('inset_mesh_probe',('inset_probe','inset_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'inset_mesh_primed': ('inset_mesh_probe',('inset_probe','inset_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'inset_mesh_ready': ('inset_mesh_probe',('inset_probe','inset_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'inset_faces': ('inset_probe',('inset_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_extrude_command': ('subd_extrude_probe', ('subd_extrude_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_offset_batch': ('subd_offset_batch', ('subd_offset_command_probe','subd_offset_command_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_offset_preferences': ('subd_offset_preferences', ('subd_offset_command_probe','subd_offset_command_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_offset_command': ('subd_offset_command_probe', ('subd_offset_command_cases','subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_offset_sdk': ('subd_offset_probe', ('subd_offset_cases','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'subd_extract_preferences': ('subd_extract_preferences', ('subd_extract_probe','subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_extract_sources': ('subd_extract_sources', ('subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','merge_edges_probe')),
    'subd_extract_command': ('subd_extract_probe', ('subd_extract_cases','subd_join_probe','subd_join_cases','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_join_history': ('subd_join_command_probe', ('subd_join_cases','subd_join_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_join_command': ('subd_join_command_probe', ('subd_join_cases','subd_join_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_join_sdk': ('subd_join_probe', ('subd_join_cases','subd_box_probe','merge_edges_probe')),
    'analysis_profile_identity': ('analysis_profile_probe', ('merge_edges_probe',)),
    'analysis_profile': ('analysis_profile_probe', ('merge_edges_probe',)),
    'analysis_nurbs_zero': ('analysis_nurbs_zero_probe', ('merge_edges_probe',)),
    'analysis_nurbs_density': ('analysis_nurbs_density_probe', ('merge_edges_probe',)),
    'nurbs_meshing': ('nurbs_meshing_probe', ('merge_edges_probe',)),
    'analysis_subd_details': ('analysis_subd_details_probe', ('merge_edges_probe',)),
    'analysis_mesh_parameters': ('analysis_mesh_parameters_probe', ('merge_edges_probe',)),
    'extract_analysis_mesh': ('extract_analysis_mesh_probe', ('join_probe','merge_edges_probe')),
    'camera_archive': ('camera_archive_probe', ('merge_edges_probe',)),
    'camera_vectors': ('camera_vectors_probe', ('merge_edges_probe',)),
    'draft_pull_direction': ('draft_pull_probe', ('draft_pull_controls','draft_pull_input','merge_edges_probe')),
    'curvature_auto_sdk': ('curvature_auto_sdk_probe', ('merge_edges_probe',)),
    'surface_analysis_input': ('analysis_input_probe', ('join_probe','merge_edges_probe')),
    'curvature_analysis': ('curvature_analysis_probe', ('curvature_analysis_controls','join_probe','merge_edges_probe')),
    'draft_angle_analysis': ('draft_angle_probe', ('join_probe', 'merge_edges_probe')),
    'subd_direction_analysis': ('direction_analysis_probe', ('subd_direction_analysis_cases','direction_analysis_input','subd_direction_probe','subd_direction_cases','subd_volume_cases','subd_area_cases','component_length_cases','subd_to_nurbs_probe','subd_box_probe','subd_join_probe','subd_join_cases','edge_loop_cases','join_probe','merge_edges_probe')),
    'direction_analysis': ('direction_analysis_probe', ('direction_analysis_input','join_probe', 'merge_edges_probe')),
    'brep_direction_command': ('brep_direction_probe', ('join_probe', 'merge_edges_probe')),
    'sweep1': ('sweep1_probe', ()),
    'curve_path_extrusion': ('curve_path_extrusion_probe', ('join_probe', 'merge_edges_probe', 'curve_definition_validation')),
    'loft_splitting': ('loft_splitting_probe', ('join_probe', 'merge_edges_probe', 'curve_definition_validation')),
    'surface_apex_command': ('surface_apex_command_probe', ('surface_apex_extrusion_probe', 'surface_extrusion_probe', 'join_probe', 'merge_edges_probe')),
    'extract_points_command': ('extract_points_probe', ('extract_points_display_cases','extract_points_subd_cases','extract_points_order_cases','extract_points_curve_range_input','extract_points_cases','extract_points_target_cases','extract_points_range_cases','extract_points_edge_range_input','owned_brep_command','extract_points_geometry','extract_points_getter','managed_component_input','extract_points_component_cases','extract_points_brep_cases','extract_points_component_input','grip_navigation_probe','grip_transform_probe','join_probe','merge_edges_probe')),
    'mesh_trim_command': ('mesh_trim_probe', ('mesh_trim_input', 'mesh_trim_getter_control_cases','mesh_trim_split_control','mesh_trim_pick_control_cases','mesh_trim_pick_trace','managed_mouse_callback','mesh_trim_curve_control_cases','mesh_trim_cutter_policy_cases','mesh_trim_snapshot_policy_cases','mesh_trim_continuation_cases','mesh_trim_control_cases', 'mesh_trim_cases', 'mesh_split_cases', 'mesh_split_ngon_policy_cases', 'mesh_split_channel_cases', 'mesh_split_selection_cases', 'mesh_split_policy_cases', 'mesh_split_sdk', 'mesh_self_intersect_cases', 'mesh_boolean_command_probe', 'mesh_boolean_command_cases', 'mesh_boolean_option_cases', 'mesh_boolean_sets_cases', 'mesh_empty_capture', 'mesh_boolean_cases', 'join_probe', 'merge_edges_probe')),
    'mesh_split_command': ('mesh_split_probe', ('mesh_split_cases', 'mesh_split_ngon_policy_cases', 'mesh_split_channel_cases', 'mesh_split_selection_cases', 'mesh_split_policy_cases', 'mesh_split_sdk', 'mesh_self_intersect_cases', 'mesh_boolean_command_probe', 'mesh_boolean_command_cases', 'mesh_boolean_option_cases', 'mesh_boolean_sets_cases', 'mesh_empty_capture', 'mesh_boolean_cases', 'join_probe', 'merge_edges_probe')),
    'mesh_self_intersect_command': ('mesh_self_intersect_probe', ('mesh_self_intersect_cases', 'mesh_boolean_command_probe', 'mesh_boolean_command_cases', 'mesh_boolean_option_cases', 'mesh_boolean_sets_cases', 'mesh_empty_capture', 'mesh_boolean_cases', 'join_probe', 'merge_edges_probe')),
    'mesh_intersect_command': ('mesh_intersect_probe', ('mesh_intersect_cases', 'mesh_intersect_coincident_cases', 'mesh_intersect_mixed_cases', 'mesh_intersect_sdk', 'mesh_boolean_command_probe', 'mesh_boolean_command_cases', 'mesh_boolean_option_cases', 'mesh_boolean_sets_cases', 'mesh_empty_capture', 'mesh_boolean_cases', 'join_probe', 'merge_edges_probe')),
    'mesh_boolean_command': ('mesh_boolean_command_probe', ('mesh_boolean_command_cases', 'mesh_boolean_option_cases', 'mesh_boolean_sets_cases', 'mesh_empty_capture', 'mesh_boolean_cases', 'join_probe', 'merge_edges_probe')),
    'mesh_boolean_sdk': ('mesh_boolean_probe', ('mesh_boolean_cases', 'merge_edges_probe')),
    'mesh_outline_command': ('mesh_outline_command_probe', ('mesh_outline_command_cases', 'mesh_outline_cases', 'mesh_outline_scale_cases', 'join_probe', 'merge_edges_probe')),
    'mesh_outline_sdk': ('mesh_outline_probe', ('mesh_outline_cases', 'mesh_outline_scale_cases', 'merge_edges_probe')),
    'unify_normals_sdk': ('unify_normals_probe', ('unify_normals_cases','unify_normals_seed_cases','unify_normals_walk_cases','mesh_channel_capture','join_probe','merge_edges_probe')),
    'unify_normals_command': ('unify_normals_probe', ('unify_normals_cases','unify_normals_seed_cases','unify_normals_walk_cases','mesh_channel_capture','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'rebuild_mesh_sdk': ('rebuild_mesh_probe', ('rebuild_mesh_cases','mesh_channel_capture','join_probe','merge_edges_probe')),
    'rebuild_mesh_command': ('rebuild_mesh_probe', ('rebuild_mesh_cases','mesh_channel_capture','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'rebuild_mesh_normals_sdk': ('rebuild_normals_probe', ('rebuild_normals_cases','mesh_channel_capture','join_probe','merge_edges_probe')),
    'rebuild_mesh_normals_command': ('rebuild_normals_probe', ('rebuild_normals_cases','mesh_channel_capture','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'triangulate_mesh_sdk': ('triangulate_probe', ('triangulate_cases','mesh_channel_capture','join_probe','merge_edges_probe')),
    'triangulate_mesh_command': ('triangulate_probe', ('triangulate_cases','mesh_channel_capture','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'nonplanar_quad_sample': ('nonplanar_sample_probe', ('nonplanar_sample_cases','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'nonplanar_quad_command': ('nonplanar_command_probe', ('nonplanar_command_cases','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'nonplanar_quad_conversion': ('nonplanar_quad_probe', ('nonplanar_quad_cases','mesh_channel_capture','join_probe','merge_edges_probe')),
    'quadrangulate_mesh_command': ('quad_command_probe', ('quad_command_cases','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'mesh_quad_conversion': ('quad_conversion_probe', ('quad_conversion_cases','mesh_channel_capture','join_probe','merge_edges_probe')),
    'insert_point_mesh_sdk': ('insert_point_mesh_sdk_probe', ('insert_point_mesh_sdk_cases','insert_point_mesh_methods','insert_point_spatial_sdk','insert_point_spatial_sdk_cases','join_probe','merge_edges_probe')),
    'insert_point_command': ('insert_point_probe', ('conic_getter_input','managed_mouse_callback','snap_environment','number_token','insert_point_cases','insert_point_holdout_cases','insert_point_ngon_cases','insert_point_mesh_shape_cases','insert_point_pick_depth_cases','insert_point_spatial_layout_cases','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'subd_spin_edge': ('subd_spin_edge_probe', ('subd_edit_scene','conic_getter_input','managed_mouse_callback','snap_environment','number_token','subd_spin_edge_cases','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'component_seams_subd': ('component_seams_subd_probe', ('component_seams_subd_cases','component_edge_seams_cases','crease_probe','crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'mesh_component_weld_commands': ('mesh_component_weld_probe', ('mesh_component_weld_cases','mesh_component_weld_followup_cases','mesh_component_weld_holdout_cases','mesh_channel_capture')),
    'mesh_component_weld_followup': ('mesh_component_weld_probe', ('mesh_component_weld_cases','mesh_component_weld_followup_cases','mesh_component_weld_holdout_cases','mesh_channel_capture')),
    'mesh_component_weld_holdout': ('mesh_component_weld_probe', ('mesh_component_weld_cases','mesh_component_weld_followup_cases','mesh_component_weld_holdout_cases','mesh_channel_capture')),
    'mesh_component_unweld_commands': ('mesh_component_unweld_command_probe', ('mesh_component_unweld_command_cases','mesh_component_unweld_primed_cases','mesh_component_unweld_verified_cases','mesh_component_unweld_holdout_cases','mesh_component_unweld_cases','mesh_unweld_channels_cases','mesh_channel_capture')),
    'mesh_component_unweld_channels': ('mesh_component_unweld_probe', ('mesh_component_unweld_cases','mesh_unweld_channels_cases','mesh_channel_capture')),
    'mesh_unweld_channels': ('mesh_unweld_channels_probe', ('mesh_unweld_channels_cases','mesh_channel_capture')),
    'mesh_weld_channels_prefixes': ('mesh_weld_channels_probe', ('mesh_weld_channels_cases','mesh_weld_channels_stress_cases','mesh_weld_channels_trace_cases','mesh_weld_channels_options_cases','mesh_channel_capture')),
    'mesh_weld_channels_followup': ('mesh_weld_channels_probe', ('mesh_weld_channels_cases','mesh_weld_channels_stress_cases','mesh_weld_channels_trace_cases','mesh_weld_channels_options_cases','mesh_channel_capture')),
    'mesh_weld_channels': ('mesh_weld_channels_probe', ('mesh_weld_channels_cases','mesh_weld_channels_stress_cases','mesh_weld_channels_trace_cases','mesh_weld_channels_options_cases','mesh_channel_capture')),
    'mesh_channels_sdk': ('mesh_channels_probe', ()),
    'control_polygon_command': ('control_polygon_probe', ('control_polygon_cases','join_probe','merge_edges_probe','owned_brep_command','shrink_face_input')),
    'soft_crease_command': ('soft_crease_probe', ('soft_crease_cases','soft_crease_chain_cases','soft_crease_followup_cases','subd_edit_scene','conic_getter_input','managed_mouse_callback','snap_environment','number_token','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'crease_command': ('crease_probe', ('crease_cases','subdivide_probe','subd_box_probe','join_probe','merge_edges_probe')),
    'creator_extrusions': ('creator_extrusions_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'creator_types': ('creator_types_probe', ('join_probe','merge_edges_probe')),
    'tosubd_command': ('tosubd_probe', ('tosubd_cases','tosubd_extrusion_cases','tosubd_info','subd_box_probe','join_probe','merge_edges_probe')),
    'subdivide_command': ('subdivide_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_ellipsoid_command': ('subd_ellipsoid_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_torus_command': ('subd_torus_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_sphere_references': ('subd_sphere_references_probe', ('subd_cylinder_references_probe','subd_box_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'subd_torus_references': ('subd_torus_references_probe', ('subd_sphere_references_probe','subd_cylinder_references_probe','subd_box_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'subd_cone_references': ('subd_cone_references_probe', ('subd_cylinder_references_probe','subd_box_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'subd_truncated_cone_references': ('subd_truncated_cone_references_probe', ('subd_cylinder_references_probe','subd_box_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'subd_truncated_cone_command': ('subd_truncated_cone_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_cone_command': ('subd_cone_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_sphere_command': ('subd_sphere_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_cylinder_references': ('subd_cylinder_references_probe', ('subd_box_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'subd_cylinder_command': ('subd_cylinder_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_plane_command': ('subd_plane_probe', ('subd_box_probe','join_probe','merge_edges_probe')),
    'subd_box_command': ('subd_box_probe', ('join_probe','merge_edges_probe')),
    'face_create_command': ('face_create_probe', ('join_probe','merge_edges_probe','face_create_input','face_draft_input','face_draft_cases')),
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
    if 'convert_extrusion_probe' in modules:
        from .convert_extrusion_cases import ASSETS
        for name in ASSETS:
            asset=Path('fixtures/extrusions')/(name+'.3dm')
            (destination/asset.parent).mkdir(parents=True,exist_ok=True)
            shutil.copyfile(source_directory/asset,destination/asset)
    if 'draft_pull_probe' in modules:
        asset=Path('fixtures/brep_direction/box.3dm')
        (destination/asset.parent).mkdir(parents=True,exist_ok=True)
        shutil.copyfile(source_directory/asset,destination/asset)
    if 'curvature_auto_sdk_probe' in modules:
        for op in request['operations']:
            if op.get('op')=='curvature_auto_sdk':
                for name in ([op['asset']] if op['asset'] is not None else [])+op.get('companions',[]):
                    asset=Path('fixtures/curvature_auto')/(name+'.3dm')
                    (destination/asset.parent).mkdir(parents=True,exist_ok=True)
                    shutil.copyfile(source_directory/asset,destination/asset)
    if 'soft_crease_probe' in modules:
        asset = Path('fixtures/soft_crease_followup_assets.json')
        (destination/asset.parent).mkdir(exist_ok=True)
        shutil.copyfile(source_directory/asset, destination/asset)
    if 'nurbs_edge_command_probe' in modules:
        from .nurbs_edge_command_probe import ASSETS
        for name in ASSETS:
            asset=Path('fixtures/subd_soft_nurbs_inputs')/(name+'.3dm')
            (destination/asset.parent).mkdir(parents=True,exist_ok=True)
            shutil.copyfile(source_directory/asset,destination/asset)
    if 'edge_analysis_mixed_probe' in modules:
        asset=Path('fixtures/brep_direction/warped.3dm')
        (destination/asset.parent).mkdir(parents=True,exist_ok=True)
        shutil.copy2(source_directory/asset,destination/asset)
    if 'subd_edge_analysis_probe' in modules:
        from .subd_edge_analysis_cases import ASSETS
        for name in ASSETS:
            asset=Path('fixtures/subd_soft_nurbs_inputs')/(name+'.3dm')
            (destination/asset.parent).mkdir(parents=True,exist_ok=True)
            shutil.copy2(source_directory/asset,destination/asset)
    if 'subd_soft_nurbs_probe' in modules:
        from .subd_soft_nurbs_cases import ASSETS,CHAIN_ASSETS
        for name in ASSETS+CHAIN_ASSETS:
            asset=Path('fixtures/subd_soft_nurbs_inputs')/(name+'.3dm')
            (destination/asset.parent).mkdir(parents=True,exist_ok=True)
            shutil.copyfile(source_directory/asset,destination/asset)
    if 'analysis_profile_probe' in modules:
        from .analysis_profile_probe import ARCHIVES,IDENTITY_ARCHIVES
        for name in ARCHIVES+IDENTITY_ARCHIVES:
            asset=Path('fixtures/analysis_profiles')/(name+'.3dm')
            (destination/asset.parent).mkdir(parents=True,exist_ok=True)
            shutil.copyfile(source_directory/asset,destination/asset)
    if 'analysis_nurbs_zero_probe' in modules:
        asset=Path('fixtures/analysis_nurbs_settings/density0.3dm')
        (destination/asset.parent).mkdir(parents=True,exist_ok=True)
        shutil.copyfile(source_directory/asset,destination/asset)
    if 'analysis_nurbs_density_probe' in modules:
        from .analysis_nurbs_density_probe import ARCHIVES
        for name in ARCHIVES:
            asset = Path('fixtures/analysis_nurbs_settings') / (name+'.3dm')
            (destination / asset.parent).mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_directory / asset, destination / asset)
    if 'analysis_subd_details_probe' in modules:
        from .analysis_subd_details_probe import ARCHIVES
        for name in ARCHIVES:
            asset = Path('fixtures/analysis_subd_settings') / (name + '.3dm')
            (destination / asset.parent).mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_directory / asset, destination / asset)
    if 'extract_analysis_mesh_probe' in modules or 'brep_direction_probe' in modules or 'direction_analysis_probe' in modules or 'draft_angle_probe' in modules or 'curvature_analysis_probe' in modules:
        for op in request['operations']:
            if op.get('op') not in ('extract_analysis_mesh','brep_direction_command','direction_analysis','draft_angle_analysis','curvature_analysis'): continue
            if op['source'] in ('line','mesh','point','panels') or op['source'].startswith('subd_'):continue
            if op.get('op')=='extract_analysis_mesh' and op['source'] in ('plane','extrusion','block','block_multi'):continue
            asset = Path('fixtures/brep_direction')/(op['source']+'.3dm')
            (destination/asset.parent).mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_directory/asset, destination/asset)
    if 'mesh_channels_probe' in modules:
        asset = Path('fixtures/mesh_channel_assets.json')
        (destination/asset.parent).mkdir(exist_ok=True)
        shutil.copyfile(source_directory/asset, destination/asset)
