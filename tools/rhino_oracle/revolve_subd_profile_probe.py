"""Independent public SDK diagnostics for the SubD revolution profile policy."""
if __package__:
    from .revolve_subd_cases import pilot, profiles
else:
    from revolve_subd_cases import pilot, profiles


def request(more=False):
    curves={r['id'].split('_')[0]:r['curve'] for r in pilot()}
    if more:curves={"sdk_"+name:curve for name,curve in profiles().items()}
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='revolve_subd_profile',id=name,curve=curve) for name,curve in sorted(curves.items())])


def validate(op):
    import json
    owned={r['id']:r for r in request()['operations']+request(True)['operations']}
    try:valid=json.dumps(op,sort_keys=True,allow_nan=False)==json.dumps(owned[op['id']],sort_keys=True,allow_nan=False)
    except (KeyError,TypeError,ValueError):valid=False
    if not valid:raise ValueError('fixed SubD profile diagnostic required')


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import subd_value
    G=host['Rhino'].Geometry
    curve=host['_nurbs_curve_from_definition'](op['curve']);friendly=None;surface=None;subd=None
    try:
        friendly=G.NurbsCurve.CreateSubDFriendly(curve)
        if friendly is None:raise ValueError('native friendly curve failed')
        line=G.Line(G.Point3d(0.,0.,0.),G.Point3d(0.,0.,1.))
        surface=G.RevSurface.Create(curve,line,0.,6.283185307179586)
        subd=G.SubD.CreateFromSurface(surface,G.SubDFromSurfaceMethods.SubDFriendlyFit,True)
        return dict(friendly=host['_nurbs_curve_definition'](friendly),surface_subd=subd_value(subd,host) if subd is not None else None),0
    finally:
        for g in [subd,surface,friendly,curve]:
            if g is not None:g.Dispose()
