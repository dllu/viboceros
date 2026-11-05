"""Public curve frames and planar cap definitions without document changes."""
if __package__:
    from .apex_splitting_cases import validate
else:
    from apex_splitting_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    G=host['Rhino'].Geometry
    curve=host['_nurbs_curve_from_definition'](op['curve']);breps=[];simple=None
    def frame(c):
        ok,p=c.TryGetPlane(tolerance['absolute'])
        xyz=lambda v:[v.X,v.Y,v.Z]
        return dict(origin=xyz(p.Origin),x=xyz(p.XAxis),y=xyz(p.YAxis),z=xyz(p.Normal))if ok else None
    try:
        original=frame(curve)
        breps=list(G.Brep.CreatePlanarBreps(curve,tolerance['absolute'])or[])
        simple=curve.Simplify(G.CurveSimplifyOptions.All,tolerance['absolute'],tolerance['angular'])
        caps=[host['_nurbs_surface_definition'](b.Faces[0].ToNurbsSurface())for b in breps]
        curve.Reverse()
        return dict(original=original,reversed=frame(curve),simplified=frame(simple)if simple else None,caps=caps),0
    finally:
        for b in breps:b.Dispose()
        if simple:simple.Dispose()
        curve.Dispose()
