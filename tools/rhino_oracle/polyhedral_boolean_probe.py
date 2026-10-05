"""Closed public SDK recipes chaining nonconvex, holed and multi-shell solids."""
import re

CASES = ('concave', 'hole', 'cavity', 'island', 'disjoint_shells',
         'two_holes', 'singular_two_holes', 'rounded_uv', 'reversed_hole', 'coplanar_concave')
OPERATIONS = ('union', 'intersection', 'difference')


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('polyhedral Boolean probes require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case', 'operation'}
                or op['op'] != 'polyhedral_boolean' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES or op['operation'] not in OPERATIONS):
            raise ValueError('invalid polyhedral Boolean recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='polyhedral_boolean', id='polyhedral_'+case+'_'+operation,
             case=case, operation=operation)
        for case in CASES for operation in OPERATIONS])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino = host['Rhino']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('polyhedral Boolean probes require idle execution')
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):
        raise ValueError('polyhedral Boolean probes require an empty owned document')
    G = Rhino.Geometry
    xyz = host['_xyz']
    owned = []

    def keep(g):
        if g is None:
            raise ValueError('polyhedral source construction returned null')
        owned.append(g)
        return g

    def box(bounds):
        return keep(G.BoundingBox(G.Point3d(*[p[0] for p in bounds]),
                                  G.Point3d(*[p[1] for p in bounds])).ToBrep())

    def boolean(a, b, name):
        if name == 'union':
            result = G.Brep.CreateBooleanUnion([a, b], 1e-7)
        elif name == 'intersection':
            result = G.Brep.CreateBooleanIntersection(a, b, 1e-7)
        else:
            result = G.Brep.CreateBooleanDifference(a, b, 1e-7)
        return None if result is None else [keep(g) for g in result]

    def one(a, b, name):
        result = boolean(a, b, name)
        if result is None or len(result) != 1:
            raise ValueError('polyhedral source recipe must yield exactly one body')
        return result[0]

    def record(g):
        mass = G.VolumeMassProperties.Compute(g)
        area = G.AreaMassProperties.Compute(g)
        try:
            bounds = g.GetBoundingBox(True)
            faces = []
            for f in g.Faces:
                loops = []
                for loop in f.Loops:
                    points = []
                    for trim in loop.Trims:
                        p = trim.PointAtStart
                        points.append(xyz(f.PointAt(p.X, p.Y)))
                    loops.append(points)
                faces.append(dict(loops=loops))
            return dict(valid=bool(g.IsValid), solid=bool(g.IsSolid),
                        orientation=str(g.SolidOrientation),
                        volume=None if mass is None else float(mass.Volume),
                        centroid=None if mass is None else xyz(mass.Centroid),
                        area=None if area is None else float(area.Area),
                        bounds=[xyz(bounds.Min), xyz(bounds.Max)], faces=faces,
                        vertices=[xyz(v.Location) for v in g.Vertices],
                        edge_samples=[[xyz(e.PointAt(e.Domain.ParameterAt(i/8.)))
                                       for i in range(9)] for e in g.Edges])
        finally:
            if mass is not None:
                mass.Dispose()
            if area is not None:
                area.Dispose()

    try:
        case = op['case']
        a = box(((0., 3.),)*3)
        b = box(((1.5, 3.5), (1.5, 3.5), (1., 2.)))
        if case in ('concave', 'coplanar_concave'):
            a = one(a, box(((2., 4.), (1., 2.), (0., 3.))), 'union')
            if case == 'coplanar_concave':
                b = box(((2., 4.), (1., 3.), (0., 3.)))
        elif case in ('hole', 'reversed_hole', 'two_holes', 'singular_two_holes'):
            a = one(a, box(((1., 2.), (1., 2.), (-1., 4.))), 'difference')
            if case == 'reversed_hole':
                a.Flip()
            if case in ('two_holes', 'singular_two_holes'):
                interval = (2.25, 3.25) if case == 'two_holes' else (2., 3.)
                b = one(box(((1., 4.),)*3),
                        box((interval, interval, (0., 5.))), 'difference')
        elif case == 'cavity':
            inner = box(((1., 2.),)*3)
            inner.Flip()
            a.Append(inner)
        elif case == 'island':
            a = box(((0., 4.),)*3)
            inner = box(((1., 3.),)*3)
            inner.Flip()
            a.Append(inner)
            a.Append(box(((1.5, 2.5),)*3))
            b = box(((2., 5.),)*3)
        elif case == 'disjoint_shells':
            a.Append(box(((4., 5.),)*3))
            b = box(((2., 4.5),)*3)
        elif case == 'rounded_uv':
            a = one(a, box(((.5, 2.5), (-1., 4.), (-1., 4.))), 'intersection')
        if not a.IsValid or not a.IsSolid or not b.IsValid or not b.IsSolid:
            raise ValueError('polyhedral source is invalid or open')
        before = [record(a), record(b)]
        result = boolean(a, b, op['operation'])
        if [record(a), record(b)] != before:
            raise ValueError('polyhedral Boolean SDK mutated inputs')
        outputs = [] if result is None else result
        return dict(inputs=before, returned_null=result is None,
                    outputs=[record(g) for g in outputs]), 0
    finally:
        for g in reversed(owned):
            g.Dispose()
