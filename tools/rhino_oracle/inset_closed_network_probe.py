"""Owned closed-edge networks sharing their source boundary with an unselected face."""
if __package__:
    from .inset_fan_basis_probe import source as annulus
    from .inset_surface_probe import run_source as run_sdk
    from .inset_outputs_probe import run_with_definitions
else:
    from inset_fan_basis_probe import source as annulus
    from inset_surface_probe import run_source as run_sdk
    from inset_outputs_probe import run_with_definitions


def cases():
    return [dict(id='adjacent_'+mode,op='inset_closed_network',scene='hole_stadium',
                 mode=mode,distance=1.5,loose=False,crease_corners=True,
                 ignore_seams=True,split=mode!='curves',join_outputs=mode=='joined')
            for mode in ('sdk','curves','pieces','joined')]


def face_cases():
    return [dict(op,id='faces_'+op['id'],op='inset_closed_network_faces',scene='box_closed_network')
            for op in cases()]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()+face_cases()
            or type(op.get('distance')) is not float
            or any(type(op.get(k)) is not bool for k in
                   ('loose','crease_corners','ignore_seams','split','join_outputs'))):
        raise ValueError('invalid owned adjacent closed-edge network recipe')


def source(op,host,tolerance):
    G=host['Rhino'].Geometry
    b=annulus(dict(op,scene='hole_stadium'),host,tolerance)
    edges=b.DuplicateNakedEdgeCurves(False,True)
    inner=G.Curve.JoinCurves(edges,tolerance['absolute'])
    fills=[];joined=[];result=None
    try:
        if inner is None or len(inner)!=1:raise ValueError('expected one closed inner boundary')
        fills=G.Brep.CreatePlanarBreps([inner[0]],tolerance['absolute'])
        if fills is None or len(fills)!=1:raise ValueError('hole fill failed')
        joined=G.Brep.JoinBreps([b,fills[0]],tolerance['absolute'])
        if joined is None or len(joined)!=1:raise ValueError('adjacent face join failed')
        result=joined[0]
        if (not result.IsValid or result.Faces.Count!=2 or result.Edges.Count!=2
                or result.Faces[0].Loops.Count!=2
                or sum(e.TrimCount==2 for e in result.Edges)!=1):
            raise ValueError('unexpected adjacent-face topology')
        return result
    except:
        result=None
        raise
    finally:
        b.Dispose()
        for c in edges:c.Dispose()
        for c in inner or []:c.Dispose()
        for f in fills or []:f.Dispose()
        for item in joined or []:
            if item is not result:item.Dispose()


def run(op,host,iterations,tolerance):
    validate(op)
    return (run_sdk if op['mode']=='sdk' else run_with_definitions)(
        op,host,iterations,tolerance,source(op,host,tolerance))
