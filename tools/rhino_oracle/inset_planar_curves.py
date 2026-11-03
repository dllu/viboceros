"""Owned public surface insets with analytic, spline and mixed trim loops."""
if __package__:
    from .inset_surface_probe import run_source
else:
    from inset_surface_probe import run_source

SCENES = ('ellipse', 'oval', 'concave_oval', 'bezier', 'stadium',
          'mixed', 'holes', 'circle_rotated', 'annulus_narrow')


def cases():
    return [dict(id=scene + ('_loose' if loose else '_tight') + ('_crease' if crease else '_ring'),
                 op='inset_planar_curves', scene=scene, distance=0.2,
                 loose=loose, ignore_seams=True, crease_corners=crease)
            for scene in SCENES for loose in (False, True) for crease in (False, True)]


def validate(op):
    if (not isinstance(op, dict) or op not in cases()
            or any(type(op.get(k)) is not bool for k in ('loose', 'ignore_seams', 'crease_corners'))):
        raise ValueError('invalid owned planar curved inset recipe')


def source(scene, R):
    G = R.Geometry
    p = lambda x,y: G.Point3d(x,y,0)
    circle = lambda x,y,r: G.Circle(p(x,y),r).ToNurbsCurve()
    curves = []
    if scene == 'ellipse': curves = [G.Ellipse(G.Plane.WorldXY,6,4).ToNurbsCurve()]
    elif scene in ('oval', 'concave_oval'):
        points = ([(6,0),(5,4),(0,5),(-6,3),(-5,-1),(-1,-4),(4,-4),(6,-2)]
                  if scene == 'oval' else [(6,0),(4,4),(0,2),(-4,4),(-6,0),(-4,-4),(0,-2),(4,-4)])
        curves = [G.NurbsCurve.Create(True,3,[p(x,y) for x,y in points])]
    elif scene == 'bezier':
        points = [(6,0),(6,2),(3,4),(0,4),(-3,4),(-5,2),(-5,0),(-5,-2),(-3,-3),(0,-3),(3,-3),(6,-2),(6,0)]
        c = G.NurbsCurve(3,False,4,len(points))
        for i,(x,y) in enumerate(points): c.Points.SetPoint(i,p(x,y))
        for i,k in enumerate([0,0,0,1,1,1,2,2,2,3,3,3,4,4,4]): c.Knots[i] = k
        curves = [c]
    elif scene == 'stadium':
        c = G.PolyCurve()
        c.Append(G.LineCurve(p(3,2),p(-3,2)))
        c.Append(G.Arc(p(-3,2),p(-5,0),p(-3,-2)).ToNurbsCurve())
        c.Append(G.LineCurve(p(-3,-2),p(3,-2)))
        c.Append(G.Arc(p(3,-2),p(5,0),p(3,2)).ToNurbsCurve())
        curves = [c]
    elif scene == 'mixed':
        curves = [G.PolylineCurve([p(-8,-6),p(8,-6),p(8,6),p(-8,6),p(-8,-6)]),circle(0,0,2)]
    elif scene == 'holes':
        curves = [G.Ellipse(G.Plane.WorldXY,8,6).ToNurbsCurve(),circle(-3,0,1),circle(3,0,1)]
    elif scene == 'circle_rotated': curves = [circle(0,0,5)]
    elif scene == 'annulus_narrow': curves = [circle(0,0,5),circle(0,0,4)]
    else: raise ValueError('unknown planar inset source')
    try:
        outputs = G.Brep.CreatePlanarBreps(curves,1e-6)
        if outputs is None or len(outputs) != 1: raise ValueError('curved planar source failed')
        b = outputs[0]
        if scene == 'circle_rotated':
            b.Transform(G.Transform.Rotation(0.7,G.Vector3d(1,2,3),p(0,0)))
            b.Transform(G.Transform.Translation(10,-20,30))
        return b
    finally:
        for c in curves: c.Dispose()


def run(op, host, iterations, tolerance):
    validate(op)
    R = host['Rhino']
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    return run_source(op,host,iterations,tolerance,source(op['scene'],R))
