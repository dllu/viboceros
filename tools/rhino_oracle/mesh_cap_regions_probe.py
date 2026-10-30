"""Public Cap with exact input storage witnesses and complete scene history."""
try:
    from .mesh_cap_regions_cases import CASES,validate
except (ImportError,ValueError):
    from mesh_cap_regions_cases import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from copy import deepcopy
    from crease_probe import run_scene
    capture_host=dict(host)
    def mesh_value(mesh):
        value=host['_polygon_mesh_value'](mesh)
        value.update(valid=bool(mesh.IsValid),solid=bool(mesh.IsSolid),
            double_precision=bool(mesh.Vertices.UseDoublePrecisionVertices),
            boundary_edges=sum(1 for i in range(mesh.TopologyEdges.Count)if len(mesh.TopologyEdges.GetConnectedFaces(i))==1))
        return value
    capture_host['_polygon_mesh_value']=mesh_value
    return run_scene(deepcopy(CASES[op['case']]),capture_host,iterations,tolerance)
