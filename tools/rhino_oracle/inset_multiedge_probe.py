"""Owned smooth-boundary partitions, including joins swallowed by cusp loops."""
if __package__:
    from .inset_planar_curves import source as planar_source
    from .inset_surface_probe import run_source as run_sdk
    from .inset_outputs_probe import run_with_definitions
else:
    from inset_planar_curves import source as planar_source
    from inset_surface_probe import run_source as run_sdk
    from inset_outputs_probe import run_with_definitions

PARTITIONS = dict(halves=(0.5,), quarters=(0.25,0.5,0.75),
                  eighths=tuple(i/8. for i in range(1,8)),
                  clustered=(1/64.,0.25,0.5,33/64.,0.75),
                  shifted=(0.25,0.5,0.75))


def cases(sdk=False):
    configurations=[('ellipse',p,d) for p in PARTITIONS for d in (0.3,3.,4.)]
    configurations += [('bezier','quarters',d) for d in (0.3,2.2,2.8,3.5)]
    rows=[]
    for scene,partition,distance in configurations:
        for crease in (False,True):
            for mode in (('sdk',) if sdk else ('curves','pieces','joined')):
                for ignore in ((True,False) if sdk and scene=='ellipse' and partition=='quarters' and distance==3. else (True,)):
                    row=dict(id=scene+'_'+partition+'_'+str(distance)+'_'+str(crease)+'_'+mode+('' if ignore else '_seams'),
                             op='inset_multiedge_sdk' if sdk else 'inset_multiedge_command',scene=scene,
                             partition=partition,distance=distance,loose=False,crease_corners=crease)
                    if sdk:row['ignore_seams']=ignore
                    else:row.update(split=mode!='curves',join_outputs=mode=='joined')
                    rows.append(row)
    return rows


def validate(op):
    if (not isinstance(op,dict) or op not in cases()+cases(True)
            or type(op.get('distance')) is not float
            or any(type(op.get(k)) is not bool for k in ('loose','crease_corners'))):
        raise ValueError('invalid owned multi-edge Inset recipe')
    keys=('ignore_seams',) if op['op']=='inset_multiedge_sdk' else ('split','join_outputs')
    if any(type(op.get(k)) is not bool for k in keys):raise ValueError('invalid multi-edge output options')


def source(op,host,tolerance):
    R,S=host['Rhino'],host['System'];b=planar_source(op['scene'],R)
    if op['partition']=='shifted':
        c=b.Edges[0].ToNurbsCurve();b.Dispose()
        try:
            if not c.ChangeClosedCurveSeam(c.Domain.ParameterAt(0.125)):raise ValueError('source seam change failed')
            b=R.Geometry.Brep.CreatePlanarBreps([c],tolerance['absolute'])[0]
        finally:c.Dispose()
    if b.Edges.Count!=1:raise ValueError('expected one unsplit source edge')
    fractions=PARTITIONS[op['partition']];domain=b.Edges[0].Domain
    b.Edges.SplitEdgeAtParameters(0,S.Array[S.Double]([domain.ParameterAt(t) for t in fractions]))
    b.Compact();b.SetTolerancesBoxesAndFlags()
    if not b.IsValid or b.Edges.Count!=len(fractions)+1:raise ValueError('source edge partition failed')
    return b


def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino']
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    b=source(op,host,tolerance)
    return (run_sdk if op['op']=='inset_multiedge_sdk' else run_with_definitions)(op,host,iterations,tolerance,b)
