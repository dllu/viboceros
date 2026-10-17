"""Bounded authored Parabola and Parabola3Pt public command inputs."""
import math
import re


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'parabola_command':
        raise ValueError('invalid parabola recipe')
    if set(op) - {'op', 'id', 'command', 'actions', 'construction_plane', 'setup'}:
        raise ValueError('unknown parabola field')
    if not isinstance(op.get('id'), str) or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z', op['id']) is None:
        raise ValueError('invalid parabola id')
    command = op.get('command')
    if command not in ('Parabola', 'Parabola3Pt'):
        raise ValueError('invalid parabola command')

    def point(p):
        return isinstance(p, list) and len(p) == 3 and all(
            type(v) in (int, float) and not (math.isnan(v) or math.isinf(v)) and abs(v) <= 1e6
            for v in p)

    options = {'MarkFocus=Yes', 'MarkFocus=No'}
    if command == 'Parabola':
        options.update(('Focus', 'Vertex', 'Half=Yes', 'Half=No'))
    else:
        options.update('Mode=' + v for v in ('Focus', 'ThroughPoint', 'Vertex'))
        options.update('PickOrder=' + v for v in ('Default', 'EndsFirst', 'EndsLast'))
    for field in ('actions', 'setup'):
        if field not in op and field == 'setup':
            continue
        actions = op.get(field)
        if not isinstance(actions, list) or not 1 <= len(actions) <= 20:
            raise ValueError('invalid parabola actions')
        if any(not (isinstance(a, str) and a in options or point(a)) for a in actions):
            raise ValueError('invalid parabola input')
    if 'construction_plane' in op and (
            not isinstance(op['construction_plane'], list)
            or len(op['construction_plane']) != 3
            or not all(point(p) for p in op['construction_plane'])):
        raise ValueError('invalid parabola construction plane')


def run(op, host, iterations, tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    return run_primitive(op, host, iterations, tolerance, op['command'])
