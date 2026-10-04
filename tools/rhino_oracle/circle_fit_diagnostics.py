"""Closed public circle/plane fits for conditioning and plane-frame diagnostics."""
if __package__:
    from .circle_fit_probe import request as original_request, validate as validate_points
else:
    from circle_fit_probe import request as original_request, validate as validate_points


def validate(op):
    if not isinstance(op, dict) or op.get("op") != "circle_fit_diagnostics":
        raise ValueError("invalid circle fitting diagnostic")
    translated = dict(op)
    translated["op"] = "circle_fit_points"
    validate_points(translated)


def request():
    rows = []
    def add(points):
        rows.append(dict(op="circle_fit_diagnostics", id="circle-fit-diag-" + str(len(rows)), points=points))
    for op in original_request()["operations"]:
        add(op["points"])
    for epsilon in (1e-2, 1e-4, 1e-6, 1e-8, 1e-10, 1e-12):
        add([[0., 0., 0.], [1., 0., 0.], [2., epsilon, 0.], [3., 0., 0.]])
    base = [[0., 0., 0.], [1., 0., 0.], [2., 1e-8, 0.], [3., 0., 0.]]
    for scale in (.001, 1000.):
        add([[scale*x, scale*y, scale*z] for x,y,z in base])
    # Exact axis permutations preserve the tiny departure from collinearity.
    for perm in ((2, 0, 1), (1, 2, 0)):
        add([[p[i] for i in perm] for p in base])
    cardinal = [[2., 0., 0.], [0., 2., 0.], [-2., 0., 0.], [0., -2., 0.]]
    add(cardinal + [[0., 0., 0.]])
    add(cardinal + [[0., 0., 0.]] * 4)
    add(cardinal + [[0., 0., .5]])
    add(cardinal + [[0., 0., 3.]])
    add(cardinal + [[.1, 0., 0.]])
    add(list(reversed(cardinal + [[0., 0., 0.]])))
    for op in rows:
        validate(op)
    return dict(protocol_version=1, iterations=1, operations=rows)


def run(op, host):
    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):
        raise ValueError("diagnostic requires empty owned document")
    points = System.Array[Rhino.Geometry.Point3d]([host["_point"](p) for p in op["points"]])
    ok, circle = Rhino.Geometry.Circle.TryFitCircleToPoints(points)
    status, plane = Rhino.Geometry.Plane.FitPlaneToPoints(points)
    def frame(plane):
        return dict(origin=host["_xyz"](plane.Origin), x=host["_xyz"](plane.XAxis),
                    y=host["_xyz"](plane.YAxis), normal=host["_xyz"](plane.Normal))
    c = frame(circle.Plane) if ok else None
    if c is not None:
        c["radius"] = circle.Radius
    return dict(circle_success=bool(ok), circle=c, plane_status=str(status),
                plane=frame(plane) if str(status) == "Success" else None), 0
