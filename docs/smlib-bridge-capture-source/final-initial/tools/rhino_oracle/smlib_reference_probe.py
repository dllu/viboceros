"""Fixed public Rhino SDK references for the optional SMLib Rust bridge."""
import math
import re

CASES = (
    'box', 'box_union', 'box_intersection', 'box_difference',
    'cylinder_through_hole', 'sphere_cavity', 'hemisphere_pocket', 'perforated_plate',
    'rational_curve_unit', 'rational_curve_shifted', 'rational_polyline',
)


def validate(op):
    if (
        set(op) != {'op', 'id', 'case'} or op.get('op') != 'smlib_reference'
        or not isinstance(op.get('id'), (str, type(u'')))
        or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
        or not isinstance(op.get('case'), (str, type(u'')))
        or op['case'] not in CASES
    ):
        raise ValueError('invalid SMLib reference recipe')


def run(op, host):
    validate(op)
    Rhino = host['Rhino']
    G = Rhino.Geometry
    if Rhino.Commands.Command.InCommand():
        raise ValueError('SMLib references require idle Rhino')
    owned = []

    def keep(value):
        if value is None:
            raise ValueError('native geometry construction failed')
        owned.append(value)
        return value

    def box(origin, size):
        lo = G.Point3d(*origin)
        hi = G.Point3d(*[origin[i] + size[i] for i in range(3)])
        return keep(G.Brep.CreateFromBox(G.BoundingBox(lo, hi)))

    def sphere(center, radius):
        return keep(G.Sphere(G.Point3d(*center), radius).ToBrep())

    def cylinder(base, radius, height):
        plane = G.Plane(G.Point3d(*base), G.Vector3d.ZAxis)
        return keep(G.Cylinder(G.Circle(plane, radius), height).ToBrep(True, True))

    def boolean(a, b, operation):
        functions = [G.Brep.CreateBooleanUnion, G.Brep.CreateBooleanIntersection, G.Brep.CreateBooleanDifference]
        results = functions[operation]([a, b], 1e-6) if operation == 0 else functions[operation](a, b, 1e-6)
        if results is None or len(results) != 1:
            raise ValueError('expected one native Boolean result')
        return keep(results[0])

    try:
        case = op['case']
        if case.startswith('rational_'):
            if case == 'rational_polyline':
                definition = dict(degree=1, control_points=[[0., 0., 0., 1.], [2., 0., 0., 4.], [2., 3., 0., 2.]], knots=[0., 0., 0.4, 1., 1.])
            else:
                a, b = (100., 105.) if case == 'rational_curve_shifted' else (0., 1.)
                definition = dict(degree=2, control_points=[[1., 0., 0., 1.], [1., 1., 0., math.sqrt(0.5)], [0., 1., 0., 1.]], knots=[a, a, a, b, b, b])
            definition['control_points'] = [dict(point=cp[:3], weight=cp[3]) for cp in definition['control_points']]
            curve = keep(host['_nurbs_curve_from_definition'](definition))
            samples = [host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i / 128.))) for i in range(129)]
            return dict(definition=host['_nurbs_curve_definition'](curve), samples=samples), 0
        size = [12., 12., 1.5] if case == 'perforated_plate' else [10., 10., 10.]
        solid = box([0., 0., 0.], size)
        if case.startswith('box_'):
            other = box([5., 2., 3.], [10., 10., 10.])
            solid = boolean(solid, other, {'box_union': 0, 'box_intersection': 1, 'box_difference': 2}[case])
        elif case == 'cylinder_through_hole':
            solid = boolean(solid, cylinder([5., 5., -1.], 2., 12.), 2)
        elif case in ('sphere_cavity', 'hemisphere_pocket'):
            solid = boolean(solid, sphere([5., 5., 5. if case == 'sphere_cavity' else 10.], 2.), 2)
        elif case == 'perforated_plate':
            for x in range(4):
                for y in range(4):
                    solid = boolean(solid, cylinder([1.5 + 3. * x, 1.5 + 3. * y, -1.], 0.75, 3.5), 2)
        bounds = solid.GetBoundingBox(True)
        properties = keep(G.VolumeMassProperties.Compute(solid))
        return dict(valid=bool(solid.IsValid), manifold=bool(solid.IsSolid), volume=float(properties.Volume), bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]), 0
    finally:
        for item in reversed(owned):
            item.Dispose()
