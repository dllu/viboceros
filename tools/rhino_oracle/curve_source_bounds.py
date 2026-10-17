"""Bounded line/analytic/NURBS curve and surface sources for owned getter probes."""
import json
import math


def validate(curves, name):
    if not isinstance(curves,list) or len(curves)>4:
        raise ValueError('invalid curve sources')
    if globals().get('__package__'):
        from .geometry_validity_probe import validate as source_validator
    else:
        from geometry_validity_probe import validate as source_validator
    def finite(value):
        return type(value) in (int,float) and abs(value)<=1e6 and not (math.isnan(value) or math.isinf(value))
    def point(value):
        return isinstance(value,list) and len(value)==3 and all(finite(v) for v in value)
    def bound_source(curve, depth=0):
        if depth > 8 or not isinstance(curve, dict):
            raise ValueError('invalid curve source nesting')
        kind = curve.get('type')
        if depth and kind in ('surface', 'box_brep'):
            raise ValueError('curve segment required')
        if kind not in ('line', 'nurbs', 'arc', 'circle', 'ellipse', 'polyline', 'polycurve', 'surface', 'box_brep'):
            raise ValueError('invalid curve rail source')
        if len(json.dumps(curve)) > 65536:
            raise ValueError('curve source size limit')

        def coordinates(node,depth=0):
            if depth>32:
                raise ValueError('curve source nesting limit')
            if isinstance(node, dict):
                for value in node.values():
                    coordinates(value,depth+1)
            elif isinstance(node, list):
                for value in node:
                    coordinates(value,depth+1)
            elif type(node) in (int, float) and (not finite(node)):
                raise ValueError('curve source coordinate limit')
        coordinates(curve)
        source_validator(dict(op='geometry_validity', id=name, source=curve))
        if kind=='line' and (not point(curve.get('start')) or not point(curve.get('end'))):
            raise ValueError('invalid curve line endpoints')
        if kind=='arc' and (not isinstance(curve.get('points'),list) or len(curve['points'])!=3 or not all(point(p) for p in curve['points'])):
            raise ValueError('invalid curve arc points')
        if kind in ('circle','ellipse'):
            axes=('x_axis','normal') if kind=='circle' else ('x_axis','y_axis')
            sizes=('radius',) if kind=='circle' else ('radius_x','radius_y')
            if not point(curve.get('center')) or not all(point(curve.get(k)) for k in axes) or not all(finite(curve.get(k)) and curve[k]>0 for k in sizes):
                raise ValueError('invalid analytic curve frame or radii')
        if kind in ('surface', 'nurbs'):
            controls = curve.get('control_points')
            if not isinstance(controls, list) or not 2 <= len(controls) <= 4096 or any((not isinstance(c, dict) or not point(c.get('point')) or (not finite(c.get('weight', 1.0))) or (c.get('weight', 1.0) <= 0) for c in controls)):
                raise ValueError('invalid curve source controls')
            axes = ('u', 'v') if kind == 'surface' else ('',)
            counts = []
            for axis in axes:
                suffix = '_' + axis if axis else ''
                degree = curve.get('degree' + suffix)
                count = curve.get('control_point_count' + suffix) if axis else len(controls)
                knots = curve.get('knots' + suffix)
                if type(degree) is not int or not 1 <= degree <= 16 or type(count) is not int or (not degree < count <= 64) or (not isinstance(knots, list)) or (len(knots) != count + degree + 1) or (not all((finite(k) for k in knots))) or any((a > b for a, b in zip(knots, knots[1:]))):
                    raise ValueError('invalid curve source chart')
                counts.append(count)
            if kind == 'surface' and len(controls) != counts[0] * counts[1]:
                raise ValueError('curve surface control layout')
        if kind == 'polyline' and (not isinstance(curve.get('vertices'), list) or not 2 <= len(curve['vertices']) <= 4096 or (not all((point(p) for p in curve['vertices'])))):
            raise ValueError('invalid curve polyline')
        if kind == 'box_brep' and (not point(curve.get('min')) or not point(curve.get('max')) or any((a >= b for a, b in zip(curve['min'], curve['max'])))):
            raise ValueError('invalid curve box')
        if kind == 'polycurve':
            segments = curve.get('segments')
            if not isinstance(segments, list) or not 1 <= len(segments) <= 16:
                raise ValueError('invalid curve polycurve')
            for segment in segments:
                bound_source(segment, depth + 1)
    for source in curves:
        bound_source(source)
