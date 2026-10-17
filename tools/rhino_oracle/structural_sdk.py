"""Host-side validation and worker companions for owned structural SDK probes."""
from importlib import import_module
from pathlib import Path
import shutil

# The validator module is also the worker entry. Dependencies stay beside it in
# the isolated job directory so standalone Rhino imports resolve transitively.
PROBES = {
    'ellipse_command': ('ellipse_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
    'circle_around': ('circle_around_probe', ('getter_recipe','curve_source_bounds','geometry_validity_probe','curve_primitive_probe','join_probe','merge_edges_probe','conic_getter_input','managed_mouse_callback','snap_environment')),
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
