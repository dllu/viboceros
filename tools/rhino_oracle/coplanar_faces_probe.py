"""Owned public SDK coplanar merging; licensed capture remains pending."""
import json
import math
from pathlib import Path


def validate(operation):
    if (not isinstance(operation, dict)
            or set(operation) != {'op', 'id', 'source'}
            or operation['op'] != 'merge_coplanar_faces'
            or not isinstance(operation['id'], str)
            or not isinstance(operation['source'], dict)):
        raise ValueError('invalid coplanar merge fields')
    source = operation['source']
    if set(source) != {'type', 'vertices', 'faces'} or source['type'] != 'mesh_brep':
        raise ValueError('coplanar recipes require a mesh_brep source')
    vertices, faces = source['vertices'], source['faces']
    if (not isinstance(vertices, list) or not 3 <= len(vertices) <= 100000
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(x) not in (int, float) or not math.isfinite(x) for x in p)
                   for p in vertices)
            or not isinstance(faces, list) or not 1 <= len(faces) <= 362
            or any(not isinstance(f, list) or len(f) not in (3, 4)
                   or any(type(i) is not int or not 0 <= i < len(vertices) for i in f)
                   or len(set(f)) != len(f) for f in faces)):
        raise ValueError('invalid mesh_brep vertices or faces')


def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('coplanar_faces.json').read_text())


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    source = host['_object_source'](operation['source'], tolerance)
    try:
        if not isinstance(source, G.Brep) or not source.IsValid:
            raise ValueError('coplanar merge requires a valid B-rep')
        before = int(source.Faces.Count)
        solid = bool(source.IsSolid)
        def compute():
            output = source.DuplicateBrep()
            if output is None or output is source:
                raise ValueError('SDK B-rep copy failed')
            try:
                changed = output.MergeCoplanarFaces(tolerance['absolute'], tolerance['angular'])
                after = int(output.Faces.Count)
                if (not output.IsValid or bool(output.IsSolid) != solid or not 0 < after <= before
                        or (not changed and after != before)):
                    raise ValueError('SDK coplanar merge produced inconsistent topology')
                area_mass = G.AreaMassProperties.Compute(output)
                if area_mass is None: raise ValueError('SDK area properties failed')
                try: area = float(area_mass.Area)
                finally: area_mass.Dispose()
                volume = None
                if solid:
                    volume_mass = G.VolumeMassProperties.Compute(output)
                    if volume_mass is None: raise ValueError('SDK volume properties failed')
                    try: volume = abs(float(volume_mass.Volume))
                    finally: volume_mass.Dispose()
                if not math.isfinite(area) or area <= 0 or (solid and not math.isfinite(volume)):
                    raise ValueError('nonfinite SDK mass properties')
                bounds = output.GetBoundingBox(True)
                corners = [host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]
                if any(len(p) != 3 or any(not math.isfinite(x) for x in p) for p in corners):
                    raise ValueError('nonfinite SDK bounds')
                return dict(face_count=after, removed_faces=before-after, is_solid=solid,
                            area=area, volume=volume,
                            loop_counts=sorted(int(f.Loops.Count) for f in output.Faces),
                            bounds=corners)
            finally:
                output.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        if source is not None: source.Dispose()
