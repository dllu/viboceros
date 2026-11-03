"""Owned Loose inset controls for fit basis, distance and kink-angle policy."""
if __package__:
    from .inset_planar_curves import source
    from .inset_surface_probe import run_source
else:
    from inset_planar_curves import source
    from inset_surface_probe import run_source


def cases():
    rows = [dict(id=scene+'_'+str(angle)+'_'+str(crease),op='inset_loose_controls',scene=scene,
                 distance=0.2,loose=True,ignore_seams=True,crease_corners=crease,angle_tolerance=angle)
            for scene in ('ellipse','bezier','oval') for angle in (1e-8,0.1) for crease in (False,True)]
    rows += [dict(id='ellipse_distance_'+str(distance)+'_'+str(crease),op='inset_loose_controls',scene='ellipse',
                  distance=distance,loose=True,ignore_seams=True,crease_corners=crease,angle_tolerance=1e-6)
             for distance in (0.05,0.1,0.4) for crease in (False,True)]
    rows += [dict(id='ellipse_degree_'+str(degree)+'_'+str(crease),op='inset_loose_controls',scene='ellipse',
                  distance=0.2,loose=True,ignore_seams=True,crease_corners=crease,angle_tolerance=1e-6,degree=degree)
             for degree in (3,7) for crease in (False,True)]
    return rows


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or any(type(op.get(k)) is not bool for k in ('loose','ignore_seams','crease_corners'))):
        raise ValueError('invalid owned Loose inset control recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    R = host['Rhino']
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    b = source(op['scene'],R)
    if 'degree' in op:
        c = b.Edges[0].ToNurbsCurve(); b.Dispose()
        try:
            if not c.IncreaseDegree(op['degree']): raise ValueError('degree elevation failed')
            b = R.Geometry.Brep.CreatePlanarBreps([c],tolerance['absolute'])[0]
        finally: c.Dispose()
    return run_source(op,host,iterations,dict(tolerance,angular=op['angle_tolerance']),b)
