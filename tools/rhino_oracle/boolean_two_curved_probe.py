"""Bounded public two-object cylinder/slab cycles using the shared driver."""
import re

try:
    from boolean_two_probe import run_recipe
except ImportError:
    from .boolean_two_probe import run_recipe

BOUNDS = [[[-2., 2.], [-2., 2.], [0., 10.]], [[-3., 3.], [-3., 3.], [4., 6.]]]
SPECS = {'cycle_'+str(i): dict(bounds=BOUNDS, cycles=i) for i in range(6)}
SPECS['keep'] = dict(SPECS['cycle_0'], delete=False)
SPECS['cancel'] = dict(SPECS['cycle_2'], cancel=True)


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='boolean_two_curved', id='boolean_two_curved_'+case, case=case)
        for case in SPECS])


def validate_request(q):
    if (not isinstance(q, dict) or set(q) - {'protocol_version', 'iterations', 'operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= len(SPECS)):
        raise ValueError('curved Boolean2Objects requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'boolean_two_curved'
                or not isinstance(op['case'], str) or op['case'] not in SPECS
                or not isinstance(op['id'], str) or re.match(r'[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen):
            raise ValueError('invalid curved Boolean2Objects recipe')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    def factory(index, geometry):
        if index == 0:
            circle = geometry.Circle(geometry.Plane.WorldXY, 2.)
            return geometry.Cylinder(circle, 10.).ToBrep(True, True)
        bounds = BOUNDS[index]
        return geometry.BoundingBox(
            geometry.Point3d(*[b[0] for b in bounds]),
            geometry.Point3d(*[b[1] for b in bounds])).ToBrep()

    return run_recipe(op, host, SPECS[op['case']], validate_request, factory)
