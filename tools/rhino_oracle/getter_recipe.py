"""Bounded shared actions, sources, planes and flags for owned curve getters."""
import math
import re


def validate(op, operation, options, numeric_pattern=None, numeric_options=()):
    if not isinstance(op, dict) or op.get('op') != operation:
        raise ValueError('invalid curve getter recipe')
    if set(op) - {'op', 'id', 'actions', 'construction_plane', 'setup', 'live_getters', 'prior_point', 'curves', 'preselect', 'guide_selection'}:
        raise ValueError('unknown curve getter field')
    if not isinstance(op.get('id'), str) or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z', op['id']) is None:
        raise ValueError('invalid curve getter id')
    if type(op.get('live_getters', False)) is not bool:
        raise ValueError('invalid curve getter getter flag')
    for field in ('preselect','guide_selection'):
        if type(op.get(field,False)) is not bool:
            raise ValueError('invalid curve getter source flag')
    curves=op.get('curves',[])
    if globals().get('__package__'):
        from .curve_source_bounds import validate as validate_sources
    else:
        from curve_source_bounds import validate as validate_sources
    validate_sources(curves,op['id'])
    def finite(v):
        return type(v) in (int, float) and abs(v) <= 1e6 and not (math.isnan(v) or math.isinf(v))
    def point(p):
        return isinstance(p, list) and len(p) == 3 and all(finite(v) for v in p)
    def action(a):
        if point(a) or finite(a):
            return True
        if isinstance(a, dict):
            if set(a)=={'relative'}:
                return point(a['relative'])
            if not {'curve'}<=set(a) or set(a)-{'curve','edge','pick'} or type(a['curve']) is not int or not 0<=a['curve']<len(curves):
                return False
            surface=curves[a['curve']]['type'] in ('surface','box_brep')
            return (set(a)=={'curve','edge','pick'} and type(a['edge']) is int and 0<=a['edge']<64 and point(a['pick']) and surface) if 'edge' in a else (set(a)=={'curve'} and not surface)
        if not isinstance(a, str):
            return False
        if a in options or (numeric_pattern is not None and re.match(numeric_pattern,a) is not None):
            return True
        key,sep,value=a.partition('=')
        if not sep or key not in numeric_options:
            return False
        try:return finite(float(value))
        except ValueError:return False
    for field in ('actions', 'setup'):
        if field == 'setup' and field not in op:
            continue
        values = op.get(field)
        if not isinstance(values, list) or not 1 <= len(values) <= 32 or not all(action(a) for a in values):
            raise ValueError('invalid curve getter actions')
    if any(isinstance(a,dict) and 'edge' in a for a in op['actions']) and not op.get('live_getters'):
        raise ValueError('curve getter edge picking requires owned live input')
    if any(isinstance(a,dict) and 'curve' in a for a in op.get('setup',[])):
        raise ValueError('curve getter setup source picks are not supported')
    if 'construction_plane' in op and (not isinstance(op['construction_plane'], list) or len(op['construction_plane']) != 3 or not all(point(p) for p in op['construction_plane'])):
        raise ValueError('invalid curve getter construction plane')
    if 'prior_point' in op and not point(op['prior_point']):
        raise ValueError('invalid curve getter prior point')
