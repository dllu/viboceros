"""Bounded public Boolean SDK witnesses on owned convex polyhedral inputs.

This probes the geometry API, not interactive Boolean command workflows.
"""
import re

CASES = ('corner', 'pierce', 'disjoint', 'contained', 'contains', 'equal',
         'touch_face', 'boundary_contained', 'partial_coplanar', 'sheared',
         'tetra_box', 'tetra_tetra')
OPERATIONS = ('union', 'intersection', 'difference')
BOUNDS = dict(
    corner=((1., 3.),)*3,
    pierce=((.5, 1.5), (.5, 1.5), (-1., 3.)),
    disjoint=((4., 5.),)*3,
    contained=((.5, 1.5),)*3,
    contains=((-1., 3.),)*3,
    equal=((0., 2.),)*3,
    touch_face=((2., 4.), (0., 2.), (0., 2.)),
    boundary_contained=((0., 1.),)*3,
    partial_coplanar=((1., 3.), (0., 2.), (.5, 1.5)),
    sheared=((1., 3.),)*3,
    tetra_box=((.5, 2.),)*3,
)


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('convex Boolean probes require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case', 'operation'}
                or op['op'] != 'convex_boolean' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES or op['operation'] not in OPERATIONS):
            raise ValueError('invalid convex Boolean recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='convex_boolean', id='convex_boolean_'+case+'_'+operation,
             case=case, operation=operation)
        for case in CASES for operation in OPERATIONS])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino = host['Rhino']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('convex Boolean probes require idle execution')
    if list(doc.Objects):
        raise ValueError('convex Boolean probes require an empty owned document')
    G = Rhino.Geometry
    xyz = host['_xyz']
    owned, outputs = [], []

    def box(bounds):
        return G.BoundingBox(G.Point3d(*[p[0] for p in bounds]),
                             G.Point3d(*[p[1] for p in bounds])).ToBrep()

    def tetra(offset):
        m = G.Mesh()
        try:
            for p in ((0., 0., 0.), (3., 0., 0.), (0., 3., 0.), (0., 0., 3.)):
                m.Vertices.Add(*[p[i]+offset[i] for i in range(3)])
            for f in ((0, 2, 1), (0, 1, 3), (0, 3, 2), (1, 2, 3)):
                m.Faces.AddFace(*f)
            return G.Brep.CreateFromMesh(m, True)
        finally:
            m.Dispose()

    def record(g):
        mass = G.VolumeMassProperties.Compute(g)
        area = G.AreaMassProperties.Compute(g)
        if mass is None or area is None:
            raise ValueError('convex Boolean mass properties failed')
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
                        orientation=str(g.SolidOrientation), volume=float(mass.Volume),
                        centroid=xyz(mass.Centroid), area=float(area.Area),
                        bounds=[xyz(bounds.Min), xyz(bounds.Max)], faces=faces,
                        vertices=[xyz(v.Location) for v in g.Vertices],
                        edge_samples=[[xyz(e.PointAt(e.Domain.ParameterAt(i/8.)))
                                       for i in range(9)] for e in g.Edges])
        finally:
            mass.Dispose()
            area.Dispose()

    try:
        a = tetra((0., 0., 0.)) if op['case'].startswith('tetra_') else box(((0., 2.),)*3)
        owned.append(a)
        b = tetra((.5, .5, .5)) if op['case'] == 'tetra_tetra' else box(BOUNDS[op['case']])
        owned.append(b)
        if op['case'] == 'sheared':
            transform = G.Transform.Identity
            transform.M01 = 1.
            transform.M12 = .5
            transform.M03 = 10.
            transform.M13 = -4.
            for g in owned:
                if not g.Transform(transform):
                    raise ValueError('convex Boolean shear failed')
        before = [record(g) for g in owned]
        if op['operation'] == 'union':
            result = G.Brep.CreateBooleanUnion(owned, 1e-7)
        elif op['operation'] == 'intersection':
            result = G.Brep.CreateBooleanIntersection(a, b, 1e-7)
        else:
            result = G.Brep.CreateBooleanDifference(a, b, 1e-7)
        if result is not None:
            outputs = list(result)
        if [record(g) for g in owned] != before:
            raise ValueError('convex Boolean SDK mutated inputs')
        if any(not g.IsValid or not g.IsSolid for g in outputs):
            raise ValueError('convex Boolean SDK returned invalid/open geometry')
        return dict(inputs=before, returned_null=result is None,
                    outputs=[record(g) for g in outputs]), 0
    finally:
        for g in outputs+owned:
            if g is not None:
                g.Dispose()
