"""Independent public SDK surface/apex recipes."""
import copy
import math


def validate(operation):
    required = {'op', 'id', 'source', 'face', 'apex', 'solid'}
    if (not isinstance(operation, dict) or set(operation) != required
            or operation['op'] != 'surface_apex_extrusion'
            or type(operation['face']) is not int or operation['face'] < 0
            or type(operation['solid']) is not bool
            or not isinstance(operation['source'], dict)
            or operation['source'].get('type') not in ('surface', 'brep', 'box_brep')):
        raise ValueError('invalid surface apex extrusion fields or options')
    apex = operation['apex']
    if not isinstance(apex, list) or len(apex) != 3 or any(type(x) not in (int, float) or math.isnan(x) or math.isinf(x) for x in apex):
        raise ValueError('invalid surface apex extrusion target')


def request():
    try:
        from .surface_extrusion_probe import request as surface_request
    except ImportError:
        from surface_extrusion_probe import request as surface_request
    q = surface_request()
    sources = {o['id']: o['source'] for o in q['operations']}
    cases = [('planar', 'surface_planar_up', [1.,1.5,6.], True),
             ('planar_down', 'surface_planar_up', [1.,1.5,-6.], True),
             ('warped', 'surface_warped_up', [1.,1.5,6.], True),
             ('warped_down', 'surface_warped_up', [1.,1.5,-6.], True),
             ('warped_off_axis', 'surface_warped_up', [4.,-1.,6.], True),
             ('ring_hole', 'surface_ring_hole', [0.,0.,6.], True),
             ('ring_walls', 'surface_ring_hole', [0.,0.,6.], False),
             ('box_face', 'surface_box_face', [2.,2.,2.], True),
             ('shifted', 'surface_warped_up', [1.,1.5,6.], True)]
    q['operations'] = [dict(op='surface_apex_extrusion', id='apex_'+name, source=copy.deepcopy(sources[source]),
                            face=0, apex=apex, solid=solid) for name, source, apex, solid in cases]
    shift = [10000.,-30000.,50000.]
    for cp in q['operations'][-1]['source']['control_points']:
        cp['point'] = [a+b for a,b in zip(cp['point'], shift)]
    q['operations'][-1]['apex'] = [a+b for a,b in zip(q['operations'][-1]['apex'], shift)]
    return q


def run(operation, host, iterations, tolerance):
    validate(operation)
    G = host['Rhino'].Geometry
    source = host['_object_source'](operation['source'], tolerance)
    brep = None
    try:
        if isinstance(source, G.Surface):
            brep = source.ToBrep()
        elif isinstance(source, G.Brep):
            brep = source.DuplicateBrep()
        if brep is None or operation['face'] >= brep.Faces.Count:
            raise ValueError('invalid surface apex extrusion face')
        apex = host['_point'](operation['apex'])
        absolute = tolerance['absolute']
        def compute():
            owned = []
            def keep(value):
                if value is None:
                    raise ValueError('SDK surface apex construction failed')
                owned.append(value)
                return value
            try:
                cap = keep(brep.Faces[operation['face']].DuplicateFace(False))
                parts = [cap] if operation['solid'] else []
                edges = [edge for edge in cap.Edges if edge.Valence == G.EdgeAdjacency.Naked]
                if not edges:
                    raise ValueError('surface apex extrusion requires a boundary')
                for edge in edges:
                    curve = keep(edge.DuplicateCurve())
                    wall = keep(G.Surface.CreateExtrusionToPoint(curve, apex))
                    parts.append(keep(wall.ToBrep()))
                joined = G.Brep.JoinBreps(parts, absolute)
                if not joined:
                    raise ValueError('SDK surface apex extrusion join failed')
                for b in joined: keep(b)
                if len(joined) == 1:
                    result = joined[0]
                else:
                    result = keep(G.Brep())
                    for b in joined: result.Append(b)
                if not result.IsValid or operation['solid'] and not result.IsSolid:
                    raise ValueError('SDK surface apex extrusion is invalid or did not close')
                volume = None
                if result.IsSolid:
                    mass = keep(G.VolumeMassProperties.Compute(result))
                    volume = abs(float(mass.Volume))
                bounds = result.GetBoundingBox(True)
                return dict(breps=[dict(is_solid=bool(result.IsSolid), face_count=int(result.Faces.Count),
                    volume=volume, bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)])])
            finally:
                disposed = set()
                for value in reversed(owned):
                    if id(value) not in disposed:
                        value.Dispose()
                        disposed.add(id(value))
        return host['_measure'](iterations, compute)
    finally:
        if brep is not None: brep.Dispose()
        source.Dispose()
