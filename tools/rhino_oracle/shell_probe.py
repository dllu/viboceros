"""Public CreateShell recipes. Live licensed Rhino capture remains pending."""
import json
import math
from pathlib import Path


def validate(operation):
    if (not isinstance(operation, dict)
            or set(operation) != {'op', 'id', 'source', 'opening_points', 'thickness'}
            or operation['op'] != 'shell'
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') not in ('box_brep', 'brep')
            or not isinstance(operation['opening_points'], list)
            or not 0 < len(operation['opening_points']) <= 100000
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(x) not in (int, float) or not math.isfinite(x) for x in p)
                   for p in operation['opening_points'])
            or type(operation['thickness']) not in (int, float)
            or not math.isfinite(operation['thickness']) or operation['thickness'] <= 0):
        raise ValueError('invalid Shell fields or options')


def request():
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
                if ok and face.PointAt(u, v).DistanceTo(point) <= tolerance['absolute']:
                    matches.append(i)
            if len(matches) != 1:
                raise ValueError('opening point must identify one face interior')
            faces.add(matches[0])
        if len(faces) >= source.Faces.Count:
            raise ValueError('Shell must retain a face')
        def compute():
            results = G.Brep.CreateShell(source, sorted(faces), -operation['thickness'], tolerance['absolute'])
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
                    rows.append(dict(is_solid=True, face_count=int(brep.Faces.Count), volume=volume,
                                     bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]))
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
