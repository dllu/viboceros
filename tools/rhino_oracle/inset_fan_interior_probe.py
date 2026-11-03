"""Owned radii, distance, basis-degree and partition controls for cusp fans."""
if __package__:
    from .inset_continuity_probe import source as boundary
    from .inset_surface_probe import run_source
else:
    from inset_continuity_probe import source as boundary
    from inset_surface_probe import run_source


def cases():
    configurations=[(3.,d,2,False) for d in (1.4,1.5,1.6,1.8)]
    configurations += [(2.5,1.7,2,False),(4.,1.5,2,False),(5.,1.4,2,False)]
    configurations += [(3.,d,3,False) for d in (1.4,1.5,1.8)]+[(3.,1.5,7,False)]
    configurations += [(3.,d,2,True) for d in (1.4,1.6,1.8)]+[(3.,1.5,3,True)]
    return [dict(id=str(rx)+'_'+str(d)+'_'+str(degree)+'_'+str(split),op='inset_fan_interior',
                 scene='elliptic_stadium',radius_x=rx,distance=d,degree=degree,subdivide=split,
                 crease_corners=True,loose=False,ignore_seams=True) for rx,d,degree,split in configurations]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or type(op.get('radius_x')) is not float or type(op.get('distance')) is not float
            or type(op.get('degree')) is not int
            or any(type(op.get(k)) is not bool for k in ('subdivide','crease_corners','loose','ignore_seams'))):
        raise ValueError('invalid owned interior cusp-fan recipe')


def run(op,host,iterations,tolerance):
    validate(op);R,S=host['Rhino'],host['System'];G=R.Geometry
    b=boundary(dict(scene='elliptic_stadium',variant='plain'),host,tolerance)
    curves=[e.ToNurbsCurve() for e in b.Edges];b.Dispose();composite=G.PolyCurve()
    try:
        for i,c in enumerate(curves):
            if i in (1,3):
                plane=G.Plane(G.Point3d(-4 if i==1 else 4,0,0),G.Vector3d.ZAxis)
                if not c.Transform(G.Transform.Scale(plane,op['radius_x']/3.,1.,1.)):
                    raise ValueError('arc radius change failed')
            if c.Degree<op['degree'] and not c.IncreaseDegree(op['degree']):raise ValueError('degree elevation failed')
            if not composite.Append(c):raise ValueError('source append failed')
        outputs=G.Brep.CreatePlanarBreps([composite],tolerance['absolute'])
        if outputs is None or len(outputs)!=1:raise ValueError('source construction failed')
        b=outputs[0]
        if b.Edges.Count!=4:raise ValueError('unexpected source partitions')
        if op['subdivide']:
            for i in range(3,-1,-1):
                b.Edges.SplitEdgeAtParameters(i,S.Array[S.Double]([b.Edges[i].Domain.Mid]))
            b.Compact();b.SetTolerancesBoxesAndFlags()
    finally:
        composite.Dispose()
        for c in curves:c.Dispose()
    return run_source(op,host,iterations,tolerance,b)
