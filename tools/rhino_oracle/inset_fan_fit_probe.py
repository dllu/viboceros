"""Owned direct-offset controls for the basis underlying Inset cusp fans."""
if __package__:
    from .inset_continuity_probe import source as boundary
else:
    from inset_continuity_probe import source as boundary


def cases():
    return [dict(id=scene+'_'+str(distance)+'_'+str(fit), op='inset_fan_fit',
                 scene=scene, distance=distance, fit_tolerance=fit)
            for scene, distance in [('upper',2.),('upper',3.),('lower',2.),('left',1.5),('closed',3.)]
            for fit in (1e-4,1e-6,1e-7)]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or type(op.get('distance')) is not float or type(op.get('fit_tolerance')) is not float):
        raise ValueError('invalid owned Inset fan-fit recipe')


def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    if op['scene']=='closed':
        curve=G.Ellipse(G.Plane.WorldXY,6,4).ToNurbsCurve();side=G.Point3d(0,0,0)
    else:
        stadium=op['scene']=='left'
        b=boundary(dict(scene='elliptic_stadium' if stadium else 'asymmetric',variant='plain'),host,tolerance)
        try:curve=b.Edges[1 if stadium or op['scene']=='lower' else 0].ToNurbsCurve()
        finally:b.Dispose()
        side=G.Point3d(-4 if stadium else 0,0,0)
    define=host['_nurbs_curve_definition']
    before=define(curve)
    def compute():
        outputs=curve.Offset(side,G.Vector3d.ZAxis,op['distance'],op['fit_tolerance'],G.CurveOffsetCornerStyle.Sharp)
        try:
            return dict(before=before,source_unchanged=define(curve)==before,
                        outputs=None if outputs is None else [dict(type=c.GetType().Name,definition=define(c)) for c in outputs])
        finally:
            if outputs is not None:
                for c in outputs:c.Dispose()
    try:return host['_measure'](iterations,compute)
    finally:curve.Dispose()
