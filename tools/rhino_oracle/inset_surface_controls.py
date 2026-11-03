"""Public SDK controls for orientation, affine skew and collinear edge junctions."""
if __package__:
    from .inset_surface_probe import source, run_source
else:
    from inset_surface_probe import source, run_source


def cases():
    return [dict(id=scene + ('_loose' if loose else '_tight') + ('_crease' if crease else '_ring'),
                 op='inset_surface_controls', scene=scene, distance=0.3,
                 loose=loose, crease_corners=crease, ignore_seams=True)
            for scene in ('triangle','reversed','parallelogram','collinear')
            for loose in (False,True) for crease in (False,True)]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or any(type(op.get(k)) is not bool for k in ('loose','crease_corners','ignore_seams'))):
        raise ValueError('invalid owned surface inset control recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']
    if iterations != 1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    G = R.Geometry
    p = lambda x,y: G.Point3d(x,y,0)
    if op['scene'] == 'triangle':
        c = G.PolylineCurve([p(0,0),p(8,0),p(2,6),p(0,0)])
        try: b = G.Brep.CreatePlanarBreps([c],tolerance['absolute'])[0]
        finally: c.Dispose()
    elif op['scene'] == 'parallelogram':
        b = G.Brep.CreateFromCornerPoints(p(0,0),p(8,0),p(10,6),p(2,6),tolerance['absolute'])
    else:
        b = source('rectangle',R)
        if op['scene'] == 'reversed': b.Flip()
        else:
            b.Edges.SplitEdgeAtParameters(0,S.Array[S.Double]([4.0])); b.Compact()
            b.SetTolerancesBoxesAndFlags()
            if b.Edges.Count != 5: raise ValueError('collinear source did not retain five edges')
    return run_source(op,host,iterations,tolerance,b)
