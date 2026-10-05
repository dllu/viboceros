"""Closed public-box command witnesses for common-intersection participation."""
import itertools
import re

try:
    from compound_recipe_probe import run_recipe
except ImportError:
    from .compound_recipe_probe import run_recipe


def cube(low, high):
    return ((low, high),) * 3


SHAPES = dict(
    a=[(cube(0., 2.), False)],
    b=[(cube(1., 3.), False)],
    enclosing=[(cube(-1., 4.), False)],
    enclosing_again=[(cube(-2., 5.), False)],
    inside=[(cube(1.2, 1.8), False)],
    boundary_inside=[(cube(1.5, 2.), False)],
    touch=[(((1., 2.), (.25, .75), (.25, .75)), False)],
    face=[(((2., 3.), (1., 2.), (1., 2.)), False)],
    point=[(cube(2., 3.), False)],
    disjoint=[(cube(10., 11.), False)],
    shared_plane=[(((-1., 2.), (-1., 4.), (-1., 4.)), False)],
    # Owned dummy solids seed DeleteInput independently of each recipe.
    equal_outer=[(cube(0., 3.), False)],
    cross=[(((1.5, 3.5), (1.5, 3.5), (1., 2.)), False)],
)

SPECS = {}
for other in ('b', 'touch', 'inside', 'a'):
    for order in ((0, 1), (1, 0)):
        key = 'two_'+other+'_'+''.join(map(str, order))
        SPECS[key] = dict(geometries=['a', other], first=list(order), second=[])
for kind in ('enclosing', 'boundary_inside', 'shared_plane'):
    for order in itertools.permutations(range(3)):
        key = kind+'_'+''.join(map(str, order))
        SPECS[key] = dict(geometries=['a', 'b', kind], first=list(order), second=[])
for order in ((0, 1, 2, 3), (2, 0, 1, 3), (2, 3, 0, 1), (3, 1, 0, 2),
              (1, 3, 2, 0), (0, 2, 3, 1), (2, 1, 3, 0), (3, 2, 1, 0)):
    SPECS['double_'+''.join(map(str, order))] = dict(
        geometries=['a', 'b', 'enclosing', 'enclosing_again'], first=list(order), second=[])
for kind in ('inside', 'disjoint', 'face', 'point', 'duplicate_a', 'duplicate_b'):
    geometry = {'duplicate_a': 'a', 'duplicate_b': 'b'}.get(kind, kind)
    for order in ((0, 1, 2), (2, 0, 1), (1, 2, 0)):
        SPECS[kind+'_'+''.join(map(str, order))] = dict(
            geometries=['a', 'b', geometry], first=list(order), second=[])
for key in ('two_b_01', 'enclosing_201'):
    SPECS[key+'_pre'] = dict(SPECS[key], pre=True, history=True)
    SPECS[key+'_keep'] = dict(SPECS[key], delete=False, history=True)
for order in itertools.permutations(range(4)):
    SPECS['four_inside_'+''.join(map(str, order))] = dict(
        geometries=['a', 'b', 'inside', 'enclosing'], first=list(order), second=[])
CASES = tuple(SPECS)


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 96):
        raise ValueError('common participation requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'common_participation' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid common participation recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='common_participation', id='common_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    return run_recipe(op, host, SPECS[op['case']], SHAPES, validate_request)
