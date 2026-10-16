"""Host-side validation and worker companions for owned structural SDK probes."""
from importlib import import_module
from pathlib import Path
import shutil

# The validator module is also the worker entry. Dependencies stay beside it in
# the isolated job directory so standalone Rhino imports resolve transitively.
PROBES = {
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
