"""Bounded public Hyperbola getter recipes and exact command/history snapshots."""
import math
import re


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'hyperbola_command':
        raise ValueError('invalid hyperbola recipe')
    if set(op) - {'op', 'id', 'actions', 'construction_plane', 'setup', 'live_getters', 'curve_diagnostics'}:
        raise ValueError('unknown hyperbola field')
    if not isinstance(op.get('id'), str) or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z', op['id']) is None:
        raise ValueError('invalid hyperbola id')
    if type(op.get("live_getters", False)) is not bool:
        raise ValueError("invalid hyperbola getter flag")
    if type(op.get("curve_diagnostics", False)) is not bool:
        raise ValueError("invalid hyperbola curve diagnostics flag")
    def finite(v):
        return type(v) in (int, float) and not (math.isnan(v) or math.isinf(v)) and abs(v) <= 1e6
    def point(p):
        return isinstance(p, list) and len(p) == 3 and all(finite(v) for v in p)
    options = {'FromCoefficient', 'FromFoci', 'FromVertex', 'A', 'B', 'Enter', 'BothBranches', 'MarkFoci', 'ShowAsymptotes'}
    options.update(k + '=' + v for k in ('BothBranches', 'MarkFoci', 'ShowAsymptotes') for v in ('Yes', 'No'))
    def action(a):
        if point(a) or finite(a) and a > 0:
            return True
        if not isinstance(a, str):
            return False
        if a in options:
            return True
        match = re.match(r'\A[AB]=([0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)\Z', a)
        return match is not None and finite(float(match.group(1))) and float(match.group(1)) > 0
    for field in ('actions', 'setup'):
        if field not in op and field == 'setup':
            continue
        values = op.get(field)
        if not isinstance(values, list) or not 1 <= len(values) <= 24 or not all(action(a) for a in values):
            raise ValueError('invalid hyperbola actions')
    if 'construction_plane' in op and (
            not isinstance(op['construction_plane'], list) or len(op['construction_plane']) != 3
            or not all(point(p) for p in op['construction_plane'])):
        raise ValueError('invalid hyperbola construction plane')


def run(op, host, iterations, tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    driver=None
    if op.get('live_getters'):
        from conic_getter_input import drive
        driver=lambda guides:drive(op,host,guides,command='Hyperbola')
    return run_primitive(op, host, iterations, tolerance, 'Hyperbola',driver)
