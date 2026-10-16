"""Owned public Rhino two-rail sweeps and geometric queries, without document edits."""
import json
import math
import os
try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)


def request():
    with open(os.path.join(os.path.dirname(__file__), 'fixtures', 'two_rail_sweep.json')) as stream:
        return json.load(stream)


def finite(value):
    return not math.isnan(value) and not math.isinf(value)


def ordered_sections(rails, sections, tolerance):
    """Arrange private SDK curves using endpoint proximity and native parameters."""
    def contact(rail, p):
        ok, t = rail.ClosestPoint(p)
        if not ok: raise ValueError('SDK rail closest point failed')
        distance = rail.PointAt(t).DistanceTo(p)
        return float(t), float(distance)
    rows = []
    for curve in sections:
        forward = [contact(rails[0], curve.PointAtStart), contact(rails[1], curve.PointAtEnd)]
        backward = [contact(rails[0], curve.PointAtEnd), contact(rails[1], curve.PointAtStart)]
        if sum(p[1] for p in backward) < sum(p[1] for p in forward):
            if not curve.Reverse(): raise ValueError('SDK section reversal failed')
            forward = backward
        if max(p[1] for p in forward) > tolerance:
            raise ValueError('SDK section endpoint misses rail')
        rows.append((forward[0][0], forward[1][0], curve))
    rows.sort(key=lambda row: row[0])
    if rows[0][1] > rows[-1][1]:
        if not rails[1].Reverse(): raise ValueError('SDK rail reversal failed')
    return [row[2] for row in rows]


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'rails', 'sections', 'queries'}
            or op['op'] != 'two_rail_sweep' or not isinstance(op['id'], string_types)
            or not isinstance(op['rails'], list) or len(op['rails']) != 2
            or not isinstance(op['sections'], list) or not 2 <= len(op['sections']) <= 32
            or not isinstance(op['queries'], list) or not 1 <= len(op['queries']) <= 64):
        raise ValueError('invalid two-rail sweep fields or counts')
    def vector(p):
        return (isinstance(p, list) and len(p) == 3
                and all(type(x) in (int, float) and finite(x) for x in p))
    if not all(vector(q) for q in op['queries']):
        raise ValueError('invalid sweep query point')
    total = 0
    for c in op['rails'] + op['sections']:
        if (not isinstance(c, dict) or not {'degree', 'knots', 'control_points'} <= set(c)
                or set(c) - {'degree', 'knots', 'control_points', 'domain'}
                or type(c['degree']) is not int or not 1 <= c['degree'] <= 16
                or not isinstance(c['control_points'], list)
                or not c['degree'] < len(c['control_points']) <= 128):
            raise ValueError('invalid sweep curve layout')
        points = c['control_points']; n = len(points); p = c['degree']; knots = c['knots']
        if (not isinstance(knots, list) or len(knots) != n+p+1
                or any(type(x) not in (int, float) or not finite(x) for x in knots)
                or any(a > b for a, b in zip(knots, knots[1:])) or not knots[p] < knots[n]
                or any(not isinstance(cp, dict) or set(cp) != {'point', 'weight'} or not vector(cp['point'])
                       or type(cp['weight']) not in (int, float) or not finite(cp['weight'])
                       or cp['weight'] == 0 for cp in points)
                or any(cp['weight'] != points[0]['weight'] for cp in points)):
            raise ValueError('invalid sweep curve basis, points or weights')
        if c.get('domain') is not None:
            d = c['domain']
            if (not isinstance(d, list) or len(d) != 2
                    or any(type(x) not in (int, float) or not finite(x) for x in d)
                    or not d[0] < d[1]):
                raise ValueError('invalid sweep curve domain')
        total += n
    if total > 1024 or max(c['degree'] for c in op['rails']) >= len(op['sections']):
        raise ValueError('sweep control limit or too few sections for rail degree')


def run(op, host, iterations, tolerance):
    validate(op)
    G = host['Rhino'].Geometry
    curves = []
    try:
        for definition in op['rails'] + op['sections']:
            curve = host['_nurbs_curve_from_definition'](definition)
            if curve is None:
                raise ValueError('SDK curve construction failed')
            curves.append(curve)
        queries = [host['_point'](q) for q in op['queries']]
        def compute():
            copies = []
            results = None
            try:
                for curve in curves:
                    duplicate = curve.DuplicateCurve()
                    if duplicate is None or duplicate is curve:
                        raise ValueError('SDK curve copy failed')
                    copies.append(duplicate)
                sections = ordered_sections(copies[:2], copies[2:], tolerance['absolute'])
                results = G.Brep.CreateFromSweep(copies[0], copies[1], sections, False, tolerance['absolute'])
                if results is None or len(results) != 1 or results[0] is None:
                    raise ValueError('SDK two-rail sweep must produce one B-rep')
                output = results[0]
                if not output.IsValid or output.IsSolid:
                    raise ValueError('invalid SDK sweep output')
                closest = []
                for q in queries:
                    best = None
                    for face in output.Faces:
                        ok, u, v = face.ClosestPoint(q)
                        if not ok: continue
                        p = face.PointAt(u, v)
                        distance = p.DistanceTo(q)
                        if best is None or distance < best[0]: best = distance, p
                    if best is None: raise ValueError('SDK sweep closest point failed')
                    closest.append(host['_xyz'](best[1]))
                mass = G.AreaMassProperties.Compute(output)
                if mass is None: raise ValueError('SDK sweep area failed')
                try:
                    area = float(mass.Area)
                    if not finite(area) or area <= 0: raise ValueError('invalid SDK sweep area')
                finally: mass.Dispose()
                return dict(face_count=int(output.Faces.Count), is_solid=False, valid=True,
                            area=area, closest_points=closest)
            finally:
                for result in results or []:
                    if result is not None: result.Dispose()
                for curve in copies: curve.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        for curve in curves: curve.Dispose()
