# -*- coding: utf-8 -*-
"""Read-only plane intersections using Rhino 8 public geometry APIs."""
import math


def validate(operation):
    required = set(('op', 'id', 'sources', 'origin', 'normal'))
    if set(operation) != required:
        raise ValueError('invalid IPlane fields')
    sources = operation['sources']
    if not isinstance(sources, list) or not 1 <= len(sources) <= 64:
        raise ValueError('IPlane requires 1..64 sources')
    if any(not isinstance(s, dict) or s.get('type') in ('point', 'point_cloud') for s in sources):
        raise ValueError('unsupported IPlane source')
    for name in ('origin', 'normal'):
        value = operation[name]
        if not isinstance(value, list) or len(value) != 3 or any(type(v) not in (int, float) or math.isnan(v) or math.isinf(v) for v in value):
            raise ValueError('invalid finite plane coordinates')
    if not any(operation['normal']):
        raise ValueError('nonzero plane normal required')


def run(operation, iterations, tolerance, host):
    validate(operation)
    import Rhino
    geometry = Rhino.Geometry
    normal = host['_vector'](operation['normal'])
    if not normal.Unitize():
        raise ValueError('could not normalize plane normal')
    plane = geometry.Plane(host['_point'](operation['origin']), normal)
    if not plane.IsValid:
        raise ValueError('invalid plane')
    absolute = float(tolerance['absolute'])
    sources = []
    def intersect():
        points, records = [], []
        for source in sources:
            curves, joined, events, brep = [], [], None, None
            try:
                intersection = geometry.Intersect.Intersection
                if isinstance(source, geometry.Curve):
                    events = intersection.CurvePlane(source, plane, absolute)
                    for event in events or []:
                        if event.IsPoint:
                            points.append(host['_xyz'](event.PointA))
                        elif event.IsOverlap:
                            curve = source.Trim(event.OverlapA)
                            if curve is None:
                                raise ValueError('could not trim plane overlap')
                            curves.append(curve)
                elif isinstance(source, geometry.Mesh):
                    for polyline in intersection.MeshPlane(source, None, plane, absolute) or []:
                        curve = polyline.ToNurbsCurve()
                        if curve is None:
                            raise ValueError('could not convert mesh section')
                        curves.append(curve)
                else:
                    if isinstance(source, geometry.Surface):
                        brep = source.ToBrep()
                        if brep is None:
                            raise ValueError('could not convert section surface')
                    elif not isinstance(source, geometry.Brep):
                        raise ValueError('unsupported IPlane geometry')
                    success, results, contacts = intersection.BrepPlane(brep if brep is not None else source, plane, absolute)
                    curves.extend(results or [])
                    if not success:
                        raise ValueError('Brep plane intersection failed')
                    points.extend(host['_xyz'](p) for p in contacts or [])
                if curves:
                    joined = list(geometry.Curve.JoinCurves(curves, absolute * 2., False) or [])
                    if not joined:
                        raise ValueError('could not join plane intersections')
                for curve in joined:
                    parameters = curve.DivideByCount(32, True)
                    if parameters is None:
                        raise ValueError('could not sample plane intersection')
                    samples = [host['_xyz'](curve.PointAt(t)) for t in parameters]
                    if curve.IsClosed and len(samples) == 33:
                        samples.pop()  # The closed seam is represented once.
                    if tuple(samples[-1]) < tuple(samples[0]):
                        samples.reverse()
                    records.append(dict(samples=samples, length=float(curve.GetLength()), closed=bool(curve.IsClosed)))
            finally:
                for curve in joined + curves:
                    curve.Dispose()
                if events is not None:
                    events.Dispose()
                if brep is not None:
                    brep.Dispose()
        points.sort(key=tuple)
        records.sort(key=lambda r: tuple(r['samples'][0]))
        return dict(points=points, curves=records)
    try:
        for definition in operation['sources']:
            sources.append(host['_object_source'](definition, tolerance))
        return host['_measure'](iterations, intersect)
    finally:
        for source in sources:
            source.Dispose()
