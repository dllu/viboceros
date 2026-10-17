"""Bounded public Helix/Spiral recipes using the shared history witness."""
import math
import re

def validate(op):
    if not isinstance(op, dict) or op.get('op') not in ('helix_command', 'spiral_command'):
        raise ValueError('invalid spiral recipe')
    if set(op) - {'op', 'id', 'actions', 'construction_plane', 'setup', 'live_getters'}:
        raise ValueError('unknown spiral field')
    if not isinstance(op.get('id'), str) or re.match('\\A[A-Za-z0-9_-]{1,100}\\Z', op['id']) is None:
        raise ValueError('invalid spiral id')
    if type(op.get('live_getters', False)) is not bool:
        raise ValueError('invalid spiral getter flag')

    def finite(v):
        return type(v) in (int, float) and (not (math.isnan(v) or math.isinf(v))) and (abs(v) <= 1000000.0)

    def point(p):
        return isinstance(p, list) and len(p) == 3 and all((finite(v) for v in p))
    words = {'Vertical', 'Flat', 'Diameter', 'Radius', 'Mode', 'Turns', 'Pitch', 'ReverseTwist', 'Enter'}
    words.update(('Mode=' + mode for mode in ('Turns', 'Pitch')))
    words.update(('ReverseTwist=' + value for value in ('Yes', 'No')))

    def action(a):
        if point(a) or finite(a):
            return True
        if not isinstance(a, str):
            return False
        if a in words:
            return a != 'Flat' or op['op'] == 'spiral_command'
        match = re.match('\\A(?:Turns|Pitch)=([0-9]+(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)\\Z', a)
        return match is not None and finite(float(match.group(1))) and (float(match.group(1)) > 0)
    for field in ('actions', 'setup'):
        if field == 'setup' and field not in op:
            continue
        values = op.get(field)
        if not isinstance(values, list) or not 1 <= len(values) <= 24 or (not all((action(a) for a in values))):
            raise ValueError('invalid spiral actions')
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
