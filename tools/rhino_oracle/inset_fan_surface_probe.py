"""Owned surface-offset controls for the fitting basis behind Inset fans."""
if __package__:
    from .inset_continuity_probe import source as boundary
else:
    from inset_continuity_probe import source as boundary


def cases():
    return [dict(id=scene+'_'+str(distance)+'_'+str(fit),op='inset_fan_surface_fit',
                 scene=scene,distance=distance,fit_tolerance=fit)
            for scene,distance in [('upper',2.),('upper',3.),('lower',2.),('left',1.5)]
            for fit in (1e-4,1e-6,1e-7)]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or type(op.get('distance')) is not float or type(op.get('fit_tolerance')) is not float):
        raise ValueError('invalid owned Inset surface fan-fit recipe')


def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino']
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    stadium=op['scene']=='left'
    b=boundary(dict(scene='elliptic_stadium' if stadium else 'asymmetric',variant='plain'),host,tolerance)
    c=b.Edges[1 if stadium or op['scene']=='lower' else 0].ToNurbsCurve()
    define=host['_nurbs_curve_definition'];before=define(c)
    def compute():
        outputs=c.OffsetOnSurface(b.Faces[0],op['distance'],op['fit_tolerance'])
        try:
            return dict(before=before,source_unchanged=define(c)==before,
                        outputs=None if outputs is None else [dict(type=x.GetType().Name,definition=define(x)) for x in outputs])
        finally:
            if outputs is not None:
                for x in outputs:x.Dispose()
    try:return host['_measure'](iterations,compute)
    finally:c.Dispose();b.Dispose()
