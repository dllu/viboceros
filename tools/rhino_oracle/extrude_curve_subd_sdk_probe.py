"""Public extrusion surface and friendly SubD factories, without model edits."""
if __package__:
    from .extrude_curve_subd_cases import validate
else:
    from extrude_curve_subd_cases import validate

def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    G=host['Rhino'].Geometry
    curve=host['_nurbs_curve_from_definition'](op['curve']);simple=None
    direction=G.Vector3d(*op['direction']);start=-direction if op['both_sides'] else G.Vector3d.Zero
    path=direction*2. if op['both_sides'] else direction
    def extruded(c):
        c=c.DuplicateCurve();surface=None;subd=None
        try:
            c.Transform(G.Transform.Translation(start))
            surface=G.Surface.CreateExtrusion(c,path)
            subd=G.SubD.CreateFromSurface(surface,G.SubDFromSurfaceMethods.SubDFriendlyFit,True)
            return dict(surface=host['_nurbs_surface_definition'](surface.ToNurbsSurface()),subd=subd_value(subd,host))
        finally:
            if subd:subd.Dispose()
            if surface:surface.Dispose()
            c.Dispose()
    try:
        ok,plane=curve.TryGetPlane(tolerance['absolute'])
        simple=curve.Simplify(G.CurveSimplifyOptions.All,tolerance['absolute'],tolerance['angular'])
        return dict(plane_normal=[plane.Normal.X,plane.Normal.Y,plane.Normal.Z]if ok else None,original=extruded(curve),simplified=extruded(simple)if simple else None),0
    finally:
        if simple:simple.Dispose()
        curve.Dispose()
