"""Owned RhinoCommon surface fillets with complete source/output definitions."""
import math


def finite(x):
    return type(x) in (int, float) and not math.isnan(x) and not math.isinf(x)


try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)


def vector(p, size):
    return isinstance(p, list) and len(p) == size and all(finite(x) for x in p)


def validate(op):
    if (not isinstance(op, dict) or set(op)-{'sides'} != {'op', 'id', 'surfaces', 'picks', 'radius', 'extend', 'queries'}
            or op['op'] != 'surface_fillet' or not isinstance(op['id'], string_types)
            or not finite(op['radius']) or op['radius'] <= 0 or type(op['extend']) is not bool
            or not isinstance(op['surfaces'], list) or len(op['surfaces']) != 2
            or not isinstance(op['picks'], list) or len(op['picks']) != 2
            or not isinstance(op['queries'], list) or not 1 <= len(op['queries']) <= 64):
        raise ValueError('invalid surface fillet fields')
    sides = op.get('sides', [1, 1])
    if not isinstance(sides, list) or len(sides) != 2 or any(type(s) is not int or s not in (-1, 1) for s in sides):
        raise ValueError('invalid surface fillet sides')
    if not all(vector(p, 2) for p in op['picks']) or not all(vector(p, 3) for p in op['queries']):
        raise ValueError('invalid surface fillet picks/queries')
    for s in op['surfaces']:
        if not isinstance(s, dict) or set(s) != {'degree_u', 'degree_v', 'control_point_count_u', 'control_point_count_v',
                                               'control_points', 'knots_u', 'knots_v'}:
            raise ValueError('invalid surface fillet definition')
        counts = []
        for axis in ['u', 'v']:
            p, n, knots = s['degree_'+axis], s['control_point_count_'+axis], s['knots_'+axis]
            if (type(p) is not int or type(n) is not int or not 1 <= p <= 17 or not p < n <= 1000000
                    or not isinstance(knots, list) or len(knots) != n+p+1 or not all(finite(x) for x in knots)
                    or any(a > b for a, b in zip(knots, knots[1:])) or not knots[p] < knots[n]):
                raise ValueError('invalid surface fillet basis')
            counts.append(n)
        if (counts[0]*counts[1] > 1000000 or not isinstance(s['control_points'], list)
                or len(s['control_points']) != counts[0]*counts[1]):
            raise ValueError('surface fillet resource limit')
        for p in s['control_points']:
            if (not isinstance(p, dict) or set(p) != {'point', 'weight'} or not vector(p['point'], 3)
                    or not finite(p['weight']) or p['weight'] == 0):
                raise ValueError('invalid surface fillet control')


def run(op, host, iterations, tolerance):
    validate(op)
    G = host['Rhino'].Geometry
    surfaces, parents = [], []
    try:
        for definition in op['surfaces']:
            s = host['_nurbs_surface_from_definition'](definition)
            surfaces.append(s)
            b = s.ToBrep()
            if b is None or not b.IsValid:
                raise ValueError('SDK surface fillet parent failed')
            parents.append(b)
        before = [host['_nurbs_surface_definition'](s) for s in surfaces]
        picks = [G.Point2d(p[0], p[1]) for p in op['picks']]
        def compute():
            results = []
            try:
                results = list(G.Brep.CreateFilletSurface(parents[0].Faces[0], picks[0],
                    parents[1].Faces[0], picks[1], op['radius'], op['extend'], tolerance['absolute']) or [])
                records = []
                for b in results:
                    box = b.GetBoundingBox(True)
                    mass = G.AreaMassProperties.Compute(b)
                    if mass is None:
                        raise ValueError('SDK surface fillet mass query failed')
                    try:
                        area = float(mass.Area)
                    finally:
                        mass.Dispose()
                    records.append(dict(valid=bool(b.IsValid), is_solid=bool(b.IsSolid),
                        face_count=int(b.Faces.Count), edge_count=int(b.Edges.Count), area=area,
                        bounds=[host['_xyz'](box.Min), host['_xyz'](box.Max)],
                        closest_points=[host['_xyz'](b.ClosestPoint(host['_point'](q))) for q in op['queries']],
                        geometry=host['_interchange_brep_record'](b)))
                return dict(outputs=records, source_unchanged=before == [host['_nurbs_surface_definition'](s) for s in surfaces])
            finally:
                for b in results:
                    b.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        for b in parents:
            b.Dispose()
        for s in surfaces:
            s.Dispose()
