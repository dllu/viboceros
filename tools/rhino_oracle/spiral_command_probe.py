"""Bounded public Helix/Spiral recipes using the shared history witness."""
import math
import json
import re

def validate(op):
    if not isinstance(op, dict) or op.get('op') not in ('helix_command', 'spiral_command'):
        raise ValueError('invalid spiral recipe')
    if set(op) - {'op', 'id', 'actions', 'construction_plane', 'setup', 'live_getters', 'curves', 'preselect', 'guide_selection', 'prior_point'}:
        raise ValueError('unknown spiral field')
    if not isinstance(op.get('id'), str) or re.match('\\A[A-Za-z0-9_-]{1,100}\\Z', op['id']) is None:
        raise ValueError('invalid spiral id')
    if type(op.get('guide_selection', False)) is not bool:
        raise ValueError('invalid spiral guide selection flag')
    if type(op.get('preselect', False)) is not bool:
        raise ValueError('invalid spiral preselection flag')
    curves = op.get('curves', [])
    if not isinstance(curves, list) or len(curves) > 4:
        raise ValueError('invalid spiral rails')
    if globals().get('__package__'):
        from .geometry_validity_probe import validate as source_validator
    else:
        from geometry_validity_probe import validate as source_validator
    if type(op.get('live_getters', False)) is not bool:
        raise ValueError('invalid spiral getter flag')

    def finite(v):
        return type(v) in (int, float) and (not (math.isnan(v) or math.isinf(v))) and (abs(v) <= 1000000.0)

    def point(p):
        return isinstance(p, list) and len(p) == 3 and all((finite(v) for v in p))

    def bound_source(curve, depth=0):
        if depth > 8 or not isinstance(curve, dict):
            raise ValueError('invalid spiral source nesting')
        kind = curve.get('type')
        if depth and kind in ('surface', 'box_brep'):
            raise ValueError('curve segment required')
        if kind not in ('line', 'nurbs', 'arc', 'circle', 'ellipse', 'polyline', 'polycurve', 'surface', 'box_brep'):
            raise ValueError('invalid spiral rail source')
        source_validator(dict(op='geometry_validity', id=op['id'], source=curve))
        if len(json.dumps(curve)) > 65536:
            raise ValueError('spiral source size limit')

        def coordinates(node):
            if isinstance(node, dict):
                for value in node.values():
                    coordinates(value)
            elif isinstance(node, list):
                for value in node:
                    coordinates(value)
            elif type(node) in (int, float) and (not finite(node)):
                raise ValueError('spiral source coordinate limit')
        coordinates(curve)
        if kind in ('surface', 'nurbs'):
            controls = curve.get('control_points')
            if not isinstance(controls, list) or not 2 <= len(controls) <= 4096 or any((not isinstance(c, dict) or not point(c.get('point')) or (not finite(c.get('weight', 1.0))) or (c.get('weight', 1.0) <= 0) for c in controls)):
                raise ValueError('invalid spiral source controls')
            axes = ('u', 'v') if kind == 'surface' else ('',)
            counts = []
            for axis in axes:
                suffix = '_' + axis if axis else ''
                degree = curve.get('degree' + suffix)
                count = curve.get('control_point_count' + suffix) if axis else len(controls)
                knots = curve.get('knots' + suffix)
                if type(degree) is not int or not 1 <= degree <= 16 or type(count) is not int or (not degree < count <= 64) or (not isinstance(knots, list)) or (len(knots) != count + degree + 1) or (not all((finite(k) for k in knots))) or any((a > b for a, b in zip(knots, knots[1:]))):
                    raise ValueError('invalid spiral source chart')
                counts.append(count)
            if kind == 'surface' and len(controls) != counts[0] * counts[1]:
                raise ValueError('spiral surface control layout')
        if kind == 'polyline' and (not isinstance(curve.get('vertices'), list) or not 2 <= len(curve['vertices']) <= 4096 or (not all((point(p) for p in curve['vertices'])))):
            raise ValueError('invalid spiral polyline')
        if kind == 'box_brep' and (not point(curve.get('min')) or not point(curve.get('max')) or any((a >= b for a, b in zip(curve['min'], curve['max'])))):
            raise ValueError('invalid spiral box')
        if kind == 'polycurve':
            segments = curve.get('segments')
            if not isinstance(segments, list) or not 1 <= len(segments) <= 16:
                raise ValueError('invalid spiral polycurve')
            for segment in segments:
                bound_source(segment, depth + 1)
    for curve in curves:
        bound_source(curve)
    if 'prior_point' in op and (not point(op['prior_point'])):
        raise ValueError('invalid spiral prior point')
    words = {'AroundCurve', 'Vertical', 'Flat', 'Diameter', 'Radius', 'Mode', 'Turns', 'Pitch', 'NumPointsPerTurn', 'ReverseTwist', 'Enter'}
    words.update(('Mode=' + mode for mode in ('Turns', 'Pitch')))
    words.update(('ReverseTwist=' + value for value in ('Yes', 'No')))

    def action(a):
        if point(a) or finite(a):
            return True
        if isinstance(a, dict):
            if set(a) == {'relative'}:
                return point(a['relative'])
            if not {'curve'} <= set(a) or set(a) - {'curve', 'edge', 'pick'} or type(a['curve']) is not int or (not 0 <= a['curve'] < len(curves)):
                return False
            surface = curves[a['curve']]['type'] in ('surface', 'box_brep')
            return set(a) == {'curve', 'edge', 'pick'} and type(a['edge']) is int and (0 <= a['edge'] < 64) and point(a['pick']) and surface if 'edge' in a else set(a) == {'curve'} and (not surface)
        if not isinstance(a, str):
            return False
        if a in words:
            return a != 'Flat' or op['op'] == 'spiral_command'
        density = re.match('\\ANumPointsPerTurn=([0-9]+)\\Z', a)
        if density:
            return 5 <= int(density.group(1)) <= 128
        match = re.match('\\A(?:Turns|Pitch)=([0-9]+(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)\\Z', a)
        return match is not None and finite(float(match.group(1))) and (float(match.group(1)) > 0)
    for field in ('actions', 'setup'):
        if field == 'setup' and field not in op:
            continue
        values = op.get(field)
        if not isinstance(values, list) or not 1 <= len(values) <= 24 or (not all((action(a) for a in values))):
            raise ValueError('invalid spiral actions')
        for i, value in enumerate(values):
            if value == 'NumPointsPerTurn' and i + 1 < len(values):
                following = values[i + 1]
                if following != 'Enter' and (type(following) not in (int, float) or not 5 <= following <= 128 or int(following) != following):
                    raise ValueError('invalid spiral density getter')
        if field == 'setup' and any((isinstance(a, dict) and 'edge' in a for a in values)):
            raise ValueError('spiral setup edges are not supported by the owned picker')
    if any((isinstance(a, dict) and 'edge' in a for a in op['actions'])) and (not op.get('live_getters')):
        raise ValueError('spiral edges need owned live input')
    if 'construction_plane' in op and (not isinstance(op['construction_plane'], list) or len(op['construction_plane']) != 3 or (not all((point(p) for p in op['construction_plane'])))):
        raise ValueError('invalid spiral construction plane')

def run(op, host, iterations, tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    command = 'Helix' if op['op'] == 'helix_command' else 'Spiral'
    driver = None
    setup_driver = None
    if op.get('live_getters'):
        from conic_getter_input import drive
        driver = lambda guides: drive(op, host, guides, command=command)
        if 'setup' in op:
            setup_driver = lambda guides: drive(dict(op, actions=op['setup'], id=op['id'] + '-setup'), host, guides, command=command)
    return run_primitive(op, host, iterations, tolerance, command, driver, setup_driver)
