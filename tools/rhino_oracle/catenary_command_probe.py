"""Bounded public Catenary getter recipes and actual history witnesses."""
import math
import re


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'catenary_command':
        raise ValueError('invalid catenary recipe')
    if set(op) - {'op', 'id', 'actions', 'construction_plane', 'setup', 'live_getters', 'prior_point', 'curves', 'persistent_snaps'}:
        raise ValueError('unknown catenary field')
    if not isinstance(op.get('id'), str) or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z', op['id']) is None:
        raise ValueError('invalid catenary id')
    if type(op.get('live_getters', False)) is not bool:
        raise ValueError('invalid catenary getter flag')
    def finite(v):
        return type(v) in (int, float) and abs(v) <= 1e6 and not (math.isnan(v) or math.isinf(v))
    def point(p):
        return isinstance(p, list) and len(p) == 3 and all(finite(v) for v in p)
    curves = op.get('curves', [])
    if not isinstance(curves, list) or len(curves) > 4:
        raise ValueError('invalid catenary snap guides')
    for source in curves:
        if not isinstance(source, dict):
            raise ValueError('invalid catenary snap source')
        if source.get('type') == 'line':
            if set(source) != {'type', 'start', 'end'} or not point(source['start']) or not point(source['end']) or source['start'] == source['end']:
                raise ValueError('invalid catenary snap line')
        elif source.get('type') == 'point':
            if set(source) != {'type', 'point'} or not point(source['point']):
                raise ValueError('invalid catenary snap point')
        else:
            raise ValueError('catenary snap guides must be lines or points')
    modes = op.get('persistent_snaps', [])
    if not isinstance(modes, list) or len(modes) > 3 or any(m not in ('End', 'Point', 'Near') for m in modes) or len(set(modes)) != len(modes):
        raise ValueError('invalid catenary snap modes')
    options = {'Mode' , 'Output', 'PointCount', 'MarkApex', 'Enter'}
    options.update('Mode=' + v for v in ('ThroughPoint', 'Length', 'Parameter', 'Apex'))
    options.update('Output=' + v for v in ('Smooth', 'Polyline'))
    options.update('MarkApex=' + v for v in ('Yes', 'No'))
    def action(a):
        if point(a) or finite(a) and a != 0:
            return True
        if isinstance(a, dict):
            return set(a) == {'relative'} and point(a['relative']) or set(a) == {'click'} and point(a['click'])
        if not isinstance(a, str):
            return False
        if a in options or a in ('ThroughPoint', 'Length', 'Parameter', 'Apex', 'Smooth', 'Polyline'):
            return True
        m = re.match(r'\APointCount=([0-9]+)\Z', a)
        return m is not None and 2 <= int(m.group(1)) <= 128
    for field in ('actions', 'setup'):
        if field == 'setup' and field not in op:
            continue
        values = op.get(field)
        if not isinstance(values, list) or not 1 <= len(values) <= 32 or not all(action(a) for a in values):
            raise ValueError('invalid catenary actions')
        for i, a in enumerate(values[:-1]):
            if a == 'PointCount':
                v = values[i+1]
                if v != 'Enter' and (type(v) not in (int, float) or not 2 <= v <= 128 or int(v) != v):
                    raise ValueError('invalid catenary point count getter')
    if any(isinstance(a, dict) and 'click' in a for a in op['actions']) and not op.get('live_getters'):
        raise ValueError('catenary clicks require owned live input')
    if any(isinstance(a, dict) and 'click' in a for a in op.get('setup', [])):
        raise ValueError('catenary setup clicks are not supported')
    if 'construction_plane' in op and (not isinstance(op['construction_plane'], list) or len(op['construction_plane']) != 3 or not all(point(p) for p in op['construction_plane'])):
        raise ValueError('invalid catenary construction plane')
    if 'prior_point' in op and not point(op['prior_point']):
        raise ValueError('invalid catenary prior point')


def run(op, host, iterations, tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    driver = None
    setup_driver = None
    if op.get('live_getters'):
        from conic_getter_input import drive
        driver = lambda guides: drive(op, host, guides, command='Catenary')
        if 'setup' in op:
            setup_driver = lambda guides: drive(dict(op, actions=op['setup'], id=op['id'] + '-setup'), host, guides, command='Catenary')
    return run_primitive(op, host, iterations, tolerance, 'Catenary', driver, setup_driver)
