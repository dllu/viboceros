"""Owned off-center straight-subdivision connector controls."""
if __package__:
    from .inset_continuity_probe import source as curved_source
    from .inset_surface_probe import run_source
else:
    from inset_continuity_probe import source as curved_source
    from inset_surface_probe import run_source

def cases():
    return [dict(id=scene+'_'+edge+'_'+str(fraction),op='inset_collinear_sdk',scene=scene,
                 edge=edge,fraction=fraction,distance=0.3,loose=False,crease_corners=True,ignore_seams=True)
            for scene in ('rectangle','elliptic_stadium') for edge in ('top','bottom') for fraction in (0.25,0.5,0.75)]

def validate(op):
    if not isinstance(op,dict) or op not in cases() or type(op.get('fraction')) is not float or any(type(op.get(k)) is not bool for k in ('loose','crease_corners','ignore_seams')):
        raise ValueError('invalid owned collinear inset recipe')

def run(op,host,iterations,tolerance):
    validate(op);R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():raise ValueError('owned empty idle document required')
    if op['scene']=='rectangle':
        b=G.PlaneSurface(G.Plane.WorldXY,G.Interval(-4,4),G.Interval(-2,2)).ToBrep()
        index=2 if op['edge']=='top' else 0
    else:
        b=curved_source(dict(scene='elliptic_stadium',variant='plain'),host,tolerance)
        index=0 if op['edge']=='top' else 2
    edge=b.Edges[index];t=edge.Domain.ParameterAt(op['fraction'])
    b.Edges.SplitEdgeAtParameters(index,S.Array[S.Double]([t]));b.Compact();b.SetTolerancesBoxesAndFlags()
    if not b.IsValid or b.Vertices.Count!=5:raise ValueError('collinear source split failed')
    return run_source(op,host,iterations,tolerance,b)
