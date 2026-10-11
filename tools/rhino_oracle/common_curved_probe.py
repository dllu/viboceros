"""Fixed public sphere/box command witnesses for curved common participation."""
import re

try:
    from compound_recipe_probe import run_recipe
    from common_participation_probe import SHAPES
except ImportError:
    from .compound_recipe_probe import run_recipe
    from .common_participation_probe import SHAPES

SPECS = {
    'inscribed': dict(geometries=['a', 'sphere_one'], first=[0, 1], second=[], history=True),
    'inside': dict(geometries=['a', 'sphere_half'], first=[0, 1], second=[], history=True),
    'crossing': dict(geometries=['a', 'sphere_cross'], first=[0, 1], second=[], history=True),
    'two_sets_inscribed': dict(geometries=['a', 'sphere_one'], first=[0], second=[1], history=True),
}


def validate_request(request):
    if (type(request.get('protocol_version')) is not int or request['protocol_version'] != 1
            or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1
            or not isinstance(request.get('operations'), list)
            or not 1 <= len(request['operations']) <= len(SPECS)):
        raise ValueError('curved common probe requires bounded protocol 1 recipes')
    seen = set()
    for op in request['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'common_curved' or not isinstance(op['case'], str) or op['case'] not in SPECS
                or not isinstance(op['id'], str) or re.match(r'[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen):
            raise ValueError('invalid curved common recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='common_curved', id='curved_common_'+case, case=case) for case in SPECS])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))

    def factory(name, geometry):
        radii = dict(sphere_one=1., sphere_half=.5, sphere_cross=1.25)
        if name not in radii:
            return None
        return geometry.Sphere(geometry.Point3d(1., 1., 1.), radii[name]).ToBrep()

    return run_recipe(op, host, SPECS[op['case']], SHAPES, validate_request, factory)
