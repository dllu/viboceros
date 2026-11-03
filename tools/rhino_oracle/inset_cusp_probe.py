"""Owned inward offsets spanning curvature limits and loop removal."""
if __package__:
    from .inset_planar_curves import source as planar_source
    from .inset_surface_probe import run_source as run_sdk
    from .inset_outputs_probe import run_with_definitions
else:
    from inset_planar_curves import source as planar_source
    from inset_surface_probe import run_source as run_sdk
    from inset_outputs_probe import run_with_definitions


def cases(sdk=False):
    configurations=[('ellipse','plain',d) for d in (2.6,2.67,3.,3.5,3.99,4.)]
    configurations += [('bezier','plain',d) for d in (2.2,2.8)]
    configurations += [('oval','plain',d) for d in (1.,1.5)]
    configurations += [('ellipse','tilted',3.),('ellipse','degree7',3.)]
    rows=[]
    for scene,variant,distance in configurations:
        for crease in (False,True):
            for mode in (('sdk',) if sdk else ('curves','pieces','joined')):
                row=dict(id=scene+'_'+variant+'_'+str(distance)+'_'+str(crease)+'_'+mode,
                         op='inset_cusp_sdk' if sdk else 'inset_cusp_command',scene=scene,
                         variant=variant,distance=distance,loose=False,crease_corners=crease)
                if sdk:row['ignore_seams']=True
                else:row.update(split=mode!='curves',join_outputs=mode=='joined')
                rows.append(row)
    return rows


def validate(op):
    if (not isinstance(op,dict) or op not in cases()+cases(True)
            or any(type(op.get(k)) is not bool for k in ('loose','crease_corners'))
            or type(op.get('distance')) is not float):
        raise ValueError('invalid owned cusp Inset recipe')
    keys=('ignore_seams',) if op['op']=='inset_cusp_sdk' else ('split','join_outputs')
    if any(type(op.get(k)) is not bool for k in keys):raise ValueError('invalid cusp output options')


def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino']
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    b=planar_source(op['scene'],R)
    if op['variant']=='degree7':
        c=b.Edges[0].ToNurbsCurve();b.Dispose()
        try:
            if not c.IncreaseDegree(7):raise ValueError('degree elevation failed')
            b=R.Geometry.Brep.CreatePlanarBreps([c],tolerance['absolute'])[0]
        finally:c.Dispose()
    if op['variant']=='tilted':
        b.Transform(R.Geometry.Transform.Rotation(0.7,R.Geometry.Vector3d(1,2,3),R.Geometry.Point3d.Origin))
        b.Transform(R.Geometry.Transform.Translation(10,-20,30))
    return (run_sdk if op['op']=='inset_cusp_sdk' else run_with_definitions)(op,host,iterations,tolerance,b)
