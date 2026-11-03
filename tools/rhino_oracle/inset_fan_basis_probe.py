"""Owned outward and hole controls exposing untrimmed Inset fitting bases."""
if __package__:
    from .inset_continuity_probe import source as boundary
    from .inset_surface_probe import run_source
else:
    from inset_continuity_probe import source as boundary
    from inset_surface_probe import run_source


def cases():
    return [dict(id=scene+'_'+str(distance)+'_'+str(crease),op='inset_fan_basis',scene=scene,
                 distance=distance,crease_corners=crease,loose=False,ignore_seams=True)
            for scene,distance in [('asymmetric',-2.),('asymmetric',-3.),('elliptic_stadium',-1.5),
                                   ('hole_asymmetric',2.),('hole_asymmetric',3.),('hole_stadium',1.5)]
            for crease in (False,True)]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()+refinement() or type(op.get('distance')) is not float
            or any(type(op.get(k)) is not bool for k in ('crease_corners','loose','ignore_seams'))):
        raise ValueError('invalid owned Inset fan-basis recipe')


def source(op,host,tolerance):
    R=host['Rhino'];G=R.Geometry
    scene='elliptic_stadium' if op['scene'] in ('hole_stadium','elliptic_stadium') else 'asymmetric'
    b=boundary(dict(scene=scene,variant='plain'),host,tolerance)
    if op['scene'].startswith('hole_'):
        edges=b.DuplicateNakedEdgeCurves(True,False)
        inner=G.Curve.JoinCurves(edges,tolerance['absolute']);outer=G.Circle(G.Plane.WorldXY,30).ToNurbsCurve()
        try:
            if len(inner)!=1:raise ValueError('expected one closed hole boundary')
            outputs=G.Brep.CreatePlanarBreps([outer,inner[0]],tolerance['absolute'])
            if outputs is None or len(outputs)!=1:raise ValueError('hole construction failed')
            b.Dispose();b=outputs[0]
            if b.Faces.Count!=1 or b.Loops.Count!=2:raise ValueError('invalid hole source')
        finally:
            outer.Dispose()
            for c in edges:c.Dispose()
            for c in inner:c.Dispose()
    return b


def run(op,host,iterations,tolerance):
    validate(op)
    b=source(op,host,tolerance)
    return run_source(op,host,iterations,dict(tolerance,absolute=op.get('fit_tolerance',tolerance['absolute'])),b)


def refinement():
    rows=[dict(id=scene+'_'+str(fit),op='inset_fan_refinement',scene=scene,distance=distance,
               fit_tolerance=fit,crease_corners=True,loose=False,ignore_seams=True)
          for scene,distance in [('hole_asymmetric',2.),('hole_stadium',1.5)]
          for fit in (0.1,0.01,0.001,0.0001,0.00001,0.000001)]
    rows += [dict(rows[i],id=rows[i]['id']+'_loose',loose=True) for i in (0,6)]
    return rows
