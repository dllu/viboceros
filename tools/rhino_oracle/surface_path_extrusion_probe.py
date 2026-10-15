"""Public SDK surface/path fixtures, independent of local extrusion output."""
import copy
import math


def validate(operation):
    required = {'op', 'id', 'source', 'face', 'path', 'solid', 'reverse', 'path_length'}
    if (not isinstance(operation, dict) or set(operation) != required
            or operation['op'] != 'surface_path_extrusion'
            or type(operation['face']) is not int or operation['face'] < 0
            or type(operation['solid']) is not bool or type(operation['reverse']) is not bool
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') not in ('surface', 'brep', 'box_brep')):
        raise ValueError('invalid surface path extrusion fields or options')
    length, path = operation['path_length'], operation['path']
    if length is not None and (type(length) not in (int, float) or not math.isfinite(length) or length <= 0):
        raise ValueError('invalid path length')
    if (not isinstance(path, dict) or not isinstance(path.get('control_points'), list)
            or not 2 <= len(path['control_points']) <= 100000):
        raise ValueError('invalid surface extrusion path')


def request():
    try:
        from .surface_extrusion_probe import request as surface_request
    except ImportError:
        from surface_extrusion_probe import request as surface_request
    q = surface_request()
    sources = {o['id']: o['source'] for o in q['operations']}
    path = dict(degree=2, knots=[2.,2.,2.,5.,5.,5.],
                control_points=[dict(point=p, weight=1.) for p in [[10.,20.,30.],[10.,21.,31.],[10.,20.,32.]]])
    line = dict(degree=1, knots=[2.,2.,5.,5.],
                control_points=[dict(point=p, weight=1.) for p in [[10.,20.,30.],[10.,20.,32.]]])
    cases = [('planar', 'surface_planar_up', path, True, False, None),
             ('warped', 'surface_warped_up', path, True, False, None),
             ('reversed', 'surface_warped_up', path, True, True, None),
             ('ring_hole', 'surface_ring_hole', path, True, False, None),
             ('ring_walls', 'surface_ring_hole', path, False, False, None),
             ('half_line', 'surface_planar_up', line, True, False, 1.),
             ('shifted', 'surface_planar_up', path, True, False, None)]
    ops = [dict(op='surface_path_extrusion', id='path_'+name, source=copy.deepcopy(sources[source]),
                face=0, path=copy.deepcopy(rail), solid=solid, reverse=reverse, path_length=length)
           for name, source, rail, solid, reverse, length in cases]
    for cp in ops[-1]['source']['control_points']:
        cp['point'] = [a+b for a,b in zip(cp['point'], [10000.,-30000.,50000.])]
    q['operations'] = ops
    return q


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    source = host['_object_source'](operation['source'], tolerance)
    brep = path = partial = None
    try:
        if isinstance(source, G.Surface):
            brep = source.ToBrep()
        elif isinstance(source, G.Brep):
            brep = source.DuplicateBrep()
        if brep is None or operation['face'] >= brep.Faces.Count:
            raise ValueError('invalid surface extrusion face')
        path = host['_nurbs_curve_from_definition'](operation['path'])
        if operation['reverse'] and not path.Reverse():
            raise ValueError('could not reverse surface extrusion path')
        length = operation['path_length']
        if length is not None:
            total = path.GetLength()
            if length > total + tolerance['absolute']:
                raise ValueError('path length exceeds path')
            if length < total:
                success, end = path.LengthParameter(length)
                if not success:
                    raise ValueError('could not parameterize path length')
                partial = path.Trim(path.Domain.T0, end)
                if partial is None:
                    raise ValueError('could not trim path length')
        rail = partial if partial is not None else path
        def compute():
            result = mass = None
            try:
                result = brep.Faces[operation['face']].CreateExtrusion(rail, operation['solid'])
                if result is None or not result.IsValid:
                    raise ValueError('SDK surface path extrusion failed')
                volume = None
                if result.IsSolid:
                    mass = G.VolumeMassProperties.Compute(result)
                    if mass is None:
                        raise ValueError('SDK surface path extrusion volume failed')
                    volume = abs(float(mass.Volume))
                bounds = result.GetBoundingBox(True)
                return dict(breps=[dict(is_solid=bool(result.IsSolid), face_count=int(result.Faces.Count),
                    volume=volume, bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)])])
            finally:
                if mass is not None: mass.Dispose()
                if result is not None: result.Dispose()
        return host['_measure'](iterations, compute)
    finally:
        for owned in (partial, path, brep, source):
            if owned is not None: owned.Dispose()
