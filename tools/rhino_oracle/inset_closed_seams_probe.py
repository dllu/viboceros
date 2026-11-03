"""Owned closed-source seam locations across straight, curved and G2 stations."""
if __package__:
    from .inset_fan_basis_probe import source as annulus
    from .inset_surface_probe import run_source
else:
    from inset_fan_basis_probe import source as annulus
    from inset_surface_probe import run_source


def cases():
    return [dict(id=scene+'_'+str(fraction),op='inset_closed_seams',scene=scene,
                 seam_fraction=fraction,distance=distance,loose=False,crease_corners=True,ignore_seams=True)
            for scene,distance in [('hole_stadium',1.5),('hole_asymmetric',2.)]
            for fraction in (0.125,0.4,0.5)]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or type(op.get('seam_fraction')) is not float or type(op.get('distance')) is not float
            or any(type(op.get(k)) is not bool for k in ('loose','crease_corners','ignore_seams'))):
        raise ValueError('invalid owned closed-source seam recipe')


def run(op,host,iterations,tolerance):
    validate(op);G=host['Rhino'].Geometry
    b=annulus(op,host,tolerance);curve=b.Edges[1].ToNurbsCurve()
    outer=G.Circle(G.Plane.WorldXY,30).ToNurbsCurve()
    try:
        if not curve.ChangeClosedCurveSeam(curve.Domain.ParameterAt(op['seam_fraction'])):
            raise ValueError('closed-source seam change failed')
        outputs=G.Brep.CreatePlanarBreps([outer,curve],tolerance['absolute'])
        if outputs is None or len(outputs)!=1:raise ValueError('seam source construction failed')
        result=outputs[0]
        if not result.IsValid or result.Edges.Count!=2 or result.Faces.Count!=1:
            result.Dispose();raise ValueError('unexpected seam source topology')
    finally:
        b.Dispose();curve.Dispose();outer.Dispose()
    return run_source(op,host,iterations,tolerance,result)
