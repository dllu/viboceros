"""Public SDK chamfer recipes; native licensed capture remains pending."""
import json
import math
from pathlib import Path


def validate(operation):
    if (not isinstance(operation, dict)
            or set(operation) != {'op', 'id', 'source', 'edge_points', 'distance', 'rail_type'}
            or operation['op'] != 'chamfer_edge'
            or operation['rail_type'] not in ('DistBetweenRails', 'RollingBall')
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') not in ('box_brep', 'cylinder_brep', 'brep')
            or not isinstance(operation['edge_points'], list)
            or not 0 < len(operation['edge_points']) <= 100000
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(x) not in (int, float) or not math.isfinite(x) for x in p)
                   for p in operation['edge_points'])
            or type(operation['distance']) not in (int, float)
            or not math.isfinite(operation['distance']) or operation['distance'] <= 0):
        raise ValueError('invalid ChamferEdge fields or options')


def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('chamfer_edge.json').read_text())


def run(operation, host, iterations, tolerance):
    validate(operation)
    from System import Array, Int32, Double
    G = host['Rhino'].Geometry
    source = host['_object_source'](operation['source'], tolerance)
    try:
        if not isinstance(source, G.Brep) or not source.IsSolid:
            raise ValueError('ChamferEdge requires a solid B-rep')
        indices = set()
        for xyz in operation['edge_points']:
            point, matches = G.Point3d(*xyz), []
            for i, edge in enumerate(source.Edges):
                if len(set(edge.AdjacentFaces())) != 2: continue
                ok, t = edge.ClosestPoint(point)
                if ok and edge.PointAt(t).DistanceTo(point) <= tolerance['absolute']: matches.append(i)
            if len(matches) != 1:
                raise ValueError('edge point must identify one edge interior')
            indices.add(matches[0])
        indices = sorted(indices)
        rail = (G.RailType.DistanceBetweenRails if operation['rail_type'] == 'DistBetweenRails'
                else G.RailType.RollingBall)
        radii = Array[Double]([operation['distance']]*len(indices))
        def compute():
            outputs = list(G.Brep.CreateFilletEdges(source, Array[Int32](indices), radii, radii,
                G.BlendType.Chamfer, rail, False, tolerance['absolute'], tolerance['angular']) or [])
            try:
                if not outputs or any(not b.IsValid or not b.IsSolid for b in outputs):
                    raise ValueError('SDK ChamferEdge failed or did not produce solid walls')
                rows = []
                for brep in outputs:
                    mass = G.VolumeMassProperties.Compute(brep)
                    if mass is None: raise ValueError('SDK ChamferEdge mass properties failed')
                    try: volume = abs(float(mass.Volume))
                    finally: mass.Dispose()
                    bounds = brep.GetBoundingBox(True)
                    rows.append(dict(is_solid=True, face_count=int(brep.Faces.Count), volume=volume,
                                     bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]))
                rows.sort(key=lambda row: tuple(row['bounds'][0]))
                return dict(breps=rows)
            finally:
                seen = set()
                for value in outputs:
                    if value is not source and id(value) not in seen:
                        value.Dispose(); seen.add(id(value))
        return host['_measure'](iterations, compute)
    finally:
        if source is not None: source.Dispose()
