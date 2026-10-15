"""Owned SDK profile: CullDegenerateFaces followed by CullUnused; no command capture."""
import json
from pathlib import Path
try:
    from .geometry_validity_probe import validate as validate_source
except ImportError:
    from geometry_validity_probe import validate as validate_source


def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('cull_degenerate_mesh_faces.json').read_text())


def validate(operation):
    if (not isinstance(operation, dict) or set(operation) != {'op', 'id', 'source'}
            or operation['op'] != 'cull_degenerate_mesh_faces'
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') != 'mesh_records'):
        raise ValueError('invalid mesh cleanup operation')
    validate_source(dict(operation, op='geometry_validity'))


def run(operation, host, iterations, tolerance):
    validate(operation)
    source = host['_object_source'](operation['source'], tolerance)
    try:
        if source is None: raise ValueError('SDK mesh construction failed')
        before_faces, before_vertices = source.Faces.Count, source.Vertices.Count
        def compute():
            output = source.DuplicateMesh()
            if output is None or output is source: raise ValueError('SDK mesh copy failed')
            try:
                output.Faces.CullDegenerateFaces()
                output.Vertices.CullUnused()
                faces, vertices = output.Faces.Count, output.Vertices.Count
                quads = sum(bool(output.Faces[i].IsQuad) for i in range(faces))
                area = 0.0
                if faces:
                    mass = host['Rhino'].Geometry.AreaMassProperties.Compute(output)
                    if mass is None: raise ValueError('SDK mesh area failed')
                    try: area = float(mass.Area)
                    finally: mass.Dispose()
                return dict(empty=faces == 0, face_count=faces, vertex_count=vertices,
                            triangle_count=faces-quads, quad_count=quads,
                            removed_faces=before_faces-faces, removed_vertices=before_vertices-vertices,
                            area=area, mesh=host['_polygon_mesh_value'](output))
            finally:
                output.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        if source is not None: source.Dispose()
