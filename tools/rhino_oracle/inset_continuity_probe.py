"""Owned curvature and curve-table-sharing controls for surface Inset."""
if __package__:
    from .inset_multiedge_probe import source as partition_source
    from .inset_surface_probe import run_source as run_sdk
    from .inset_outputs_probe import run_with_definitions
else:
    from inset_multiedge_probe import source as partition_source
    from inset_surface_probe import run_source as run_sdk
    from inset_outputs_probe import run_with_definitions


def cases(sdk=False):
    configurations=[('ellipse',v,0.3,loose) for v in ('shared','independent') for loose in (False,True)]
    configurations += [('asymmetric','plain',d,False) for d in (0.3,2.,3.,3.5)]
    configurations += [('elliptic_stadium',v,d,False) for v in ('plain','split') for d in (0.3,1.5,2.)]
    rows=[]
    for scene,variant,distance,loose in configurations:
        for crease in (False,True):
            for mode in (('sdk',) if sdk else ('curves','pieces','joined')):
                op=dict(id=scene+'_'+variant+'_'+str(distance)+'_'+str(loose)+'_'+str(crease)+'_'+mode,
                        op='inset_continuity_sdk' if sdk else 'inset_continuity_command',
                        scene=scene,variant=variant,distance=distance,loose=loose,crease_corners=crease)
                if sdk:op['ignore_seams']=True
                else:op.update(split=mode!='curves',join_outputs=mode=='joined')
                rows.append(op)
    return rows


def validate(op):
    if not isinstance(op,dict) or op not in cases()+cases(True)+limits() or type(op.get('distance')) is not float:
        raise ValueError('invalid owned Inset continuity recipe')
    keys=('loose','crease_corners')+(('split','join_outputs') if op['op']=='inset_continuity_command' else ('ignore_seams',))
    if any(type(op.get(k)) is not bool for k in keys):raise ValueError('invalid continuity options')


def source(op,host,tolerance):
    R,S=host['Rhino'],host['System'];G=R.Geometry;p=lambda x,y:G.Point3d(x,y,0)
    if op['scene']=='ellipse':
        b=partition_source(dict(scene='ellipse',partition='quarters'),host,tolerance)
        if op['variant']=='independent':
            for edge in b.Edges:
                c=edge.ToNurbsCurve()
                try:
                    index=b.Curves3D.Add(c)
                    if index<0 or not edge.SetEdgeCurve(index):raise ValueError('independent edge assignment failed')
                finally:c.Dispose()
            b.Compact();b.SetTolerancesBoxesAndFlags()
        if not b.IsValid or b.Edges.Count!=4:raise ValueError('invalid ellipse partition')
        return b
    curves=[];owned=[]
    def ellipse(x,a,b):
        plane=G.Plane(p(x,0),G.Vector3d(0,0,1))
        c=G.Ellipse(plane,a,b).ToNurbsCurve();owned.append(c);return c
    if op['scene']=='asymmetric':
        top=ellipse(0,6,4);bottom=ellipse(0,6,op.get('lower_radius',3.))
        curves=[top.Trim(top.Domain.T0,top.Domain.Mid),bottom.Trim(bottom.Domain.Mid,bottom.Domain.T1)]
    else:
        left=ellipse(-4,3,2);right=ellipse(4,3,2)
        a=left.Trim(left.Domain.ParameterAt(0.25),left.Domain.ParameterAt(0.75))
        if not right.ChangeClosedCurveSeam(right.Domain.ParameterAt(0.75)):raise ValueError('stadium seam failed')
        b=right.Trim(right.Domain.T0,right.Domain.Mid)
        curves=[G.LineCurve(p(4,2),p(-4,2)),a,G.LineCurve(p(-4,-2),p(4,-2)),b]
    composite=G.PolyCurve()
    try:
        for c in curves:
            if c is None or not composite.Append(c):raise ValueError('curved source append failed')
        outputs=G.Brep.CreatePlanarBreps([composite],tolerance['absolute'])
        if outputs is None or len(outputs)!=1:raise ValueError('curved source creation failed')
        b=outputs[0]
        expected=2 if op['scene']=='asymmetric' else 4
        if b.Edges.Count!=expected:raise ValueError('unexpected curved source edge count')
        if op['variant']=='split':
            for i in range(expected-1,-1,-1):
                domain=b.Edges[i].Domain
                b.Edges.SplitEdgeAtParameters(i,S.Array[S.Double]([domain.Mid]))
            b.Compact();b.SetTolerancesBoxesAndFlags();expected*=2
        if not b.IsValid or b.Edges.Count!=expected:raise ValueError('invalid curved source')
        return b
    finally:
        composite.Dispose()
        for c in curves+owned:
            if c is not None:c.Dispose()


def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino']
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    b=source(op,host,tolerance)
    indices=[int(e.EdgeCurveIndex) if hasattr(e,'EdgeCurveIndex') else None for e in b.Edges]
    curvatures=[]
    for e in b.Edges:
        row=[]
        for t in (e.Domain.T0,e.Domain.T1):
            v=e.CurvatureAt(t);row.append([float(v.X),float(v.Y),float(v.Z)])
        curvatures.append(row)
    value,elapsed=(run_with_definitions if op['op']=='inset_continuity_command' else run_sdk)(op,host,iterations,tolerance,b)
    value['source_curve_indices']=indices;value['source_endpoint_curvatures']=curvatures
    return value,elapsed


def limits():
    return [dict(id='curvature_'+str(radius)+'_'+str(crease),op='inset_continuity_limits',scene='asymmetric',
                 variant='plain',lower_radius=radius,distance=0.3,loose=False,crease_corners=crease,ignore_seams=True)
            for radius in (3.9999,3.99,3.9,3.5) for crease in ((False,True) if radius in (3.99,3.5) else (True,))]
