"""Public apex-surface and friendly-SubD factories; no document edits."""
if __package__:
    from .extrude_apex_subd_cases import validate
else:
    from extrude_apex_subd_cases import validate

def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    G=host['Rhino'].Geometry
    curve=host['_nurbs_curve_from_definition'](op['curve']);surface=None;subd=None;nurbs=None
    try:
        surface=G.Surface.CreateExtrusionToPoint(curve,G.Point3d(*op['apex']))
        nurbs=surface.ToNurbsSurface()
        subd=G.SubD.CreateFromSurface(surface,G.SubDFromSurfaceMethods.SubDFriendlyFit,True)
        return dict(surface=host['_nurbs_surface_definition'](nurbs),subd=subd_value(subd,host)if subd is not None else None),0
    finally:
        for g in [subd,nurbs,surface,curve]:
            if g is not None:g.Dispose()
