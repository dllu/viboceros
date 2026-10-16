"""Owned structural SDK validity probes; native command/getter capture is separate."""
import json
import math
import os
try:
    string_types=(basestring,)
except NameError:
    string_types=(str,)


def request():
    with open(os.path.join(os.path.dirname(__file__),'fixtures','geometry_validity.json')) as stream:
        return json.load(stream)


def validate(operation):
    if (not isinstance(operation, dict) or set(operation) != {'op', 'id', 'source'}
            or operation['op'] != 'geometry_validity' or not isinstance(operation['id'], string_types)
            or not isinstance(operation['source'], dict) or not isinstance(operation['source'].get('type'), string_types)):
        raise ValueError('invalid geometry validity operation')
    source = operation['source']
    # The host's shared source constructors validate the remaining family-specific
    # fields. Stored meshes deliberately skip geometric validity, but retain all
    # index/count/finite checks before calling native allocation routines.
    numeric_fields = {'point', 'points', 'vertices', 'start', 'end', 'center', 'min', 'max',
                      'x_axis', 'y_axis', 'normal', 'radius', 'radius_x', 'radius_y',
                      'height', 'weight', 'knots', 'knots_u', 'knots_v', 'faces',
                      'domain', 'domain_u', 'domain_v'}
    integer_fields = {'degree', 'degree_u', 'degree_v', 'control_point_count_u', 'control_point_count_v'}
    def finite_tree(value, depth=0, numeric=False):
        if depth > 16:
            raise ValueError('geometry source nesting limit')
        if numeric and type(value) not in (int, float, list):
            raise ValueError('geometry coordinates must be numeric')
        if type(value) in (int, float) and (math.isnan(value) or math.isinf(value)):
            raise ValueError('nonfinite geometry source')
        if isinstance(value, dict):
            for key, child in value.items():
                if key in integer_fields and type(child) is not int:
                    raise ValueError('geometry counts and degrees must be integers')
                finite_tree(child, depth+1, key in numeric_fields)
        elif isinstance(value, list):
            if len(value) > 1000000: raise ValueError('geometry source count limit')
            for child in value: finite_tree(child, depth+1, numeric)
    finite_tree(source)
    if source['type'] == 'mesh_records':
        if set(source) != {'type', 'vertices', 'faces'}:
            raise ValueError('invalid stored mesh fields')
        vertices, faces = source['vertices'], source['faces']
        if (not isinstance(vertices, list) or not 3 <= len(vertices) <= 1000000
                or any(not isinstance(p, list) or len(p) != 3 or any(type(x) not in (int, float) for x in p) for p in vertices)
                or not isinstance(faces, list) or not 1 <= len(faces) <= 1000000
                or any(not isinstance(f, list) or len(f) not in (3, 4)
                       or any(type(i) is not int or not 0 <= i < len(vertices) for i in f)
                       or len(set(f)) != len(f) for f in faces)):
            raise ValueError('invalid stored mesh layout')


def run(operation, host, iterations, tolerance):
    validate(operation)
    source = host['_object_source'](operation['source'], tolerance)
    try:
        if source is None: raise ValueError('SDK geometry construction failed')
        def compute():
            output = source.Duplicate()
            if output is None or output is source:
                raise ValueError('SDK geometry copy failed')
            try:
                valid, log = output.IsValidWithLog()
                return dict(status='valid' if bool(valid) else 'invalid',
                            has_diagnostics=bool(str(log or '').strip()) or not bool(valid))
            finally:
                output.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        if source is not None: source.Dispose()
