"""Closed witnesses for interacting inward pairs and compound common sets."""
import re

try:
    from compound_intersection_probe import SHAPES as BASE_SHAPES, CUBE
    from compound_recipe_probe import run_recipe
except ImportError:
    from .compound_intersection_probe import SHAPES as BASE_SHAPES, CUBE
    from .compound_recipe_probe import run_recipe

SHAPES = dict(BASE_SHAPES)
VOIDS = dict(
    crossing=CUBE(1.5, 2.5),
    nested=CUBE(1.25, 1.75),
    equal=CUBE(1, 2),
    face=((2., 2.5), (1., 2.), (1., 2.)),
    edge=((2., 2.5), (2., 2.5), (1., 2.)),
    point=CUBE(2, 2.5),
    disjoint=CUBE(2.125, 2.625),
    boundary_nested=CUBE(1.25, 2),
)
for name, bounds in VOIDS.items():
    SHAPES['void_'+name] = [(bounds, True)]
    SHAPES['cavity_'+name] = [(CUBE(.5, 3.5), False), (bounds, True)]
SHAPES['cavity_contained_outer'] = [(CUBE(-.5, 3.5), False), (VOIDS['crossing'], True)]
SHAPES['cavity_equal_outer'] = [(CUBE(0, 3), False), (VOIDS['crossing'], True)]

SPECS = {}
for name in VOIDS:
    SPECS['sdk_'+name] = dict(geometries=['inner', 'void_'+name], sdk=True)
    SPECS['pair_'+name] = dict(geometries=['cavity', 'cavity_'+name], first=[0], second=[1])
for name in ('contained_outer', 'equal_outer'):
    SPECS['pair_'+name] = dict(geometries=['cavity', 'cavity_'+name], first=[0], second=[1])
for case in ('pair_crossing', 'pair_nested', 'pair_face'):
    source = SPECS[case]
    SPECS[case+'_reverse'] = dict(source, first=[1], second=[0])
for suffix, flags in (('pre', dict(pre=True, history=True)), ('keep', dict(delete=False, history=True))):
    SPECS['pair_crossing_'+suffix] = dict(SPECS['pair_crossing'], **flags)
for name in ('cross', 'unopened', 'contains_inner', 'inside_inner', 'equal_inner', 'enclosing', 'touches_inner'):
    SPECS['common_'+name] = dict(geometries=['cavity', name], first=[0, 1], second=[])
for name in ('crossing', 'nested', 'disjoint', 'face', 'equal', 'edge', 'point', 'boundary_nested', 'contained_outer', 'equal_outer'):
    SPECS['common_cavities_'+name] = dict(geometries=['cavity', 'cavity_'+name], first=[0, 1], second=[])
SPECS['common_island'] = dict(geometries=['island', 'cross'], first=[0, 1], second=[])
SPECS['common_cross_pre'] = dict(SPECS['common_cross'], pre=True, history=True)
SPECS['common_cross_keep'] = dict(SPECS['common_cross'], delete=False, history=True)
SPECS['common_three_cavities'] = dict(geometries=['cavity', 'cavity_crossing', 'contains_inner'], first=[0, 1, 2], second=[])
SPECS['first_multi'] = dict(geometries=['cavity', 'cavity_crossing', 'extra_cross'], first=[0, 2], second=[1])
SPECS['second_multi'] = dict(geometries=['cavity', 'cavity_crossing', 'extra_cross'], first=[0], second=[1, 2])
for name in ('crossing', 'nested', 'disjoint'):
    SPECS['common_three_enclosing_'+name] = dict(geometries=['cavity', 'cavity_'+name, 'enclosing'], first=[0, 1, 2], second=[])
SPECS['common_three_enclosing_first'] = dict(SPECS['common_three_enclosing_crossing'], first=[2, 0, 1])
SPECS['common_three_touch'] = dict(geometries=['cavity', 'touches_inner', 'enclosing'], first=[0, 1, 2], second=[])
for case in ('common_cross', 'common_touches_inner', 'common_cavities_crossing'):
    SPECS[case+'_reverse'] = dict(SPECS[case], first=[1, 0])
CASES = tuple(SPECS)


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 96):
        raise ValueError('compound pairs require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'compound_pairs' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid compound pair recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='compound_pairs', id='pairs_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    return run_recipe(op, host, SPECS[op['case']], SHAPES, validate_request)
