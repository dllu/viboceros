"""Bounded public Helix/Spiral recipes using the shared history witness."""
import math
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
    if type(op.get('live_getters', False)) is not bool:
        raise ValueError('invalid spiral getter flag')

    def finite(v):
        return type(v) in (int, float) and (not (math.isnan(v) or math.isinf(v))) and (abs(v) <= 1000000.0)

    def point(p):
        return isinstance(p, list) and len(p) == 3 and all((finite(v) for v in p))

    if globals().get('__package__'):
        from .curve_source_bounds import validate as validate_sources
    else:
        from curve_source_bounds import validate as validate_sources
    validate_sources(curves, op['id'])
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
