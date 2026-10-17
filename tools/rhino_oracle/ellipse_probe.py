"""Bounded public Ellipse getter recipes and model/history observations."""
import math
import re


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'ellipse_command':
        raise ValueError('invalid ellipse recipe')
    if set(op) - {'op', 'id', 'actions', 'construction_plane', 'setup', 'live_getters', 'prior_point'}:
        raise ValueError('unknown ellipse field')
    if not isinstance(op.get('id'), str) or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z', op['id']) is None:
        raise ValueError('invalid ellipse id')
    if type(op.get('live_getters', False)) is not bool:
        raise ValueError('invalid ellipse getter flag')
    def finite(v):
        return type(v) in (int, float) and abs(v) <= 1e6 and not (math.isnan(v) or math.isinf(v))
    def point(p):
        return isinstance(p, list) and len(p) == 3 and all(finite(v) for v in p)
    options = {'Default', 'Vertical', 'Corner', 'Diameter', 'FromFoci', 'Deformable', 'Degree', 'PointCount', 'MarkFoci', 'Enter'}
    options.update(k + '=' + v for k in ('Deformable', 'MarkFoci') for v in ('Yes', 'No'))
    def action(a):
        if point(a) or finite(a):
            return True
        if isinstance(a, dict):
            return set(a) == {'relative'} and point(a['relative'])
        if not isinstance(a, str):
            return False
        return a in options or re.match(r'\A(?:Degree|PointCount)=[0-9]{1,2}\Z', a) is not None
    for field in ('actions', 'setup'):
        if field == 'setup' and field not in op:
            continue
        values = op.get(field)
        if not isinstance(values, list) or not 1 <= len(values) <= 32 or not all(action(a) for a in values):
            raise ValueError('invalid ellipse actions')
    if 'construction_plane' in op and (not isinstance(op['construction_plane'], list) or len(op['construction_plane']) != 3 or not all(point(p) for p in op['construction_plane'])):
        raise ValueError('invalid ellipse construction plane')
    if 'prior_point' in op and not point(op['prior_point']):
        raise ValueError('invalid ellipse prior point')


def run(op, host, iterations, tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    driver = setup_driver = None
    if op.get('live_getters'):
        from conic_getter_input import drive
        driver = lambda guides: drive(op, host, guides, command='Ellipse')
        if 'setup' in op:
            setup_driver = lambda guides: drive(dict(op, actions=op['setup'], id=op['id'] + '-setup'), host, guides, command='Ellipse')
    return run_primitive(op, host, iterations, tolerance, 'Ellipse', driver, setup_driver)
