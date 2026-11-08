"""Public CreateShell recipes, compatible with Rhino's IronPython worker."""
import json
import math


def finite(value):
    return not math.isnan(value) and not math.isinf(value)


def validate(operation):
    if (not isinstance(operation, dict)
            or set(operation) - {'diagnostic', 'include_geometry'} != {'op', 'id', 'source', 'opening_points', 'thickness'}
            or type(operation.get('diagnostic', False)) is not bool
            or type(operation.get('include_geometry', False)) is not bool
            or operation['op'] != 'shell'
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') not in ('box_brep', 'cylinder_brep', 'brep')
            or not isinstance(operation['opening_points'], list)
            or not 0 < len(operation['opening_points']) <= 100000
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(x) not in (int, float) or not finite(x) for x in p)
                   for p in operation['opening_points'])
            or type(operation['thickness']) not in (int, float)
            or not finite(operation['thickness']) or operation['thickness'] <= 0):
        raise ValueError('invalid Shell fields or options')


def request():
    from pathlib import Path
    return json.loads(Path(__file__).with_name('fixtures').joinpath('shell.json').read_text())


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    source = host['_object_source'](operation['source'], tolerance)
    try:
        if not isinstance(source, G.Brep) or not source.IsSolid:
            raise ValueError('Shell requires a solid B-rep')
        faces = set()
        for xyz in operation['opening_points']:
            point = G.Point3d(*xyz)
            matches = []
            for i, face in enumerate(source.Faces):
                ok, u, v = face.ClosestPoint(point)
                if (ok and face.PointAt(u, v).DistanceTo(point) <= tolerance['absolute']
                        and face.IsPointOnFace(u, v) == G.PointFaceRelation.Interior):
                    matches.append(i)
            if len(matches) != 1:
                raise ValueError('opening point must identify one face interior')
            faces.add(matches[0])
        if len(faces) >= source.Faces.Count:
            raise ValueError('Shell must retain a face')
        if operation.get('diagnostic'):
            attempts = []
            for sign in [-1, 1]:
                owned = list(G.Brep.CreateShell(source, sorted(faces), sign * operation['thickness'], tolerance['absolute']) or [])
                try:
                    rows = []
                    for b in owned:
                        row = dict(native_valid=bool(b.IsValid), is_solid=bool(b.IsSolid),
                                   face_count=int(b.Faces.Count), solid_orientation=str(b.SolidOrientation))
                        if b.IsValid and b.IsSolid:
                            mass = G.VolumeMassProperties.Compute(b)
                            try: row['volume'] = abs(float(mass.Volume)) if mass is not None else None
                            finally:
                                if mass is not None: mass.Dispose()
                            bounds = b.GetBoundingBox(True)
                            row['bounds'] = [host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]
                        rows.append(row)
                    attempts.append(dict(distance=sign * operation['thickness'], breps=rows))
                finally:
                    for b in owned:
                        if b is not source: b.Dispose()
            return dict(source_orientation=str(source.SolidOrientation), faces_to_remove=sorted(faces),
                        attempts=attempts), 0
        def compute():
            results = G.Brep.CreateShell(source, sorted(faces), operation['thickness'], tolerance['absolute'])
            owned = list(results or [])
            try:
                if not owned or any(not b.IsValid or not b.IsSolid for b in owned):
                    raise ValueError('SDK Shell failed or did not produce solid walls')
                rows = []
                for brep in owned:
                    mass = G.VolumeMassProperties.Compute(brep)
                    if mass is None: raise ValueError('SDK Shell mass properties failed')
                    try: volume = abs(float(mass.Volume))
                    finally: mass.Dispose()
                    bounds = brep.GetBoundingBox(True)
                    row = dict(is_solid=True, face_count=int(brep.Faces.Count), volume=volume,
                               bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)])
                    if operation.get('include_geometry'):
                        row['geometry'] = host['_interchange_brep_record'](brep, include_samples=False)
                    rows.append(row)
                rows.sort(key=lambda row: tuple(row["bounds"][0]))
                return dict(breps=rows)
            finally:
                seen = set()
                for value in owned:
                    if id(value) not in seen and value is not source:
                        value.Dispose(); seen.add(id(value))
        return host['_measure'](iterations, compute)
    finally:
        source.Dispose()
