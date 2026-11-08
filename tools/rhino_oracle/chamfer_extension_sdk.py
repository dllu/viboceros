"""Public extension API controls for nonmeeting Chamfer branch diagnostics."""
from math import sqrt


def request():
    operations=[]
    transforms={
        'xy':lambda p:p,
        'tilted_y':lambda p:[7.+.6*p[0],-2.+p[1],3.+.8*p[0]],
        'tilted_x':lambda p:[p[0],.6*p[1],.8*p[1]],
    }
    axes={'xy':([1.,0.,0.],[0.,0.,1.]),'tilted_y':([.6,0.,.8],[-.8,0.,.6]),'tilted_x':([1.,0.,0.],[0.,-.8,.6])}
    for label,transform in transforms.items():
        source=dict(type='arc',points=[transform(p)for p in [[2.,5.,0.],[sqrt(2.),5.+sqrt(2.),0.],[0.,7.,0.]]])
        x_axis,normal=axes[label]
        boundary=dict(type='circle',center=transform([0.,0.,0.]),radius=5.,x_axis=x_axis,normal=normal)
        for side in ['Start','End','Both']:
            for style in ['Line','Arc','Smooth']:
                operations.append(dict(op='chamfer_extension_sdk',id=label+'-'+side+'-'+style,source=source,boundary=boundary,side=side,style=style))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


def validate(op):
    if not isinstance(op,dict) or op.get('op')!='chamfer_extension_sdk' or op.get('side')not in ['Start','End','Both'] or op.get('style')not in ['Line','Arc','Smooth'] or op.get('source',{}).get('type')!='arc' or op.get('boundary',{}).get('type')!='circle':
        raise ValueError('invalid prescribed extension API diagnostic')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects):raise ValueError('empty owned document required')
    source=host['_object_source'](op['source'],tolerance)
    boundary=host['_object_source'](op['boundary'],tolerance)
    extended=None
    def observe(curve):
        if curve is None:return None
        result=dict(native_type=curve.GetType().Name,native_valid=bool(curve.IsValid),domain=[float(curve.Domain.T0),float(curve.Domain.T1)],ends=[host['_xyz'](curve.PointAtStart),host['_xyz'](curve.PointAtEnd)])
        hits=G.Intersect.Intersection.CurveCurve(curve,boundary,tolerance['absolute'],tolerance['absolute'])
        result['intersections']=[dict(parameter=float(h.ParameterA),point=host['_xyz'](h.PointA))for h in hits]
        if hits is not None:hits.Dispose()
        return result
    try:
        before=host['_nurbs_curve_definition'](source)
        extended=source.Extend(getattr(G.CurveEnd,op['side']),getattr(G.CurveExtensionStyle,op['style']),S.Array[G.GeometryBase]([boundary]))
        return dict(source=observe(source),extended=observe(extended),source_unchanged=before==host['_nurbs_curve_definition'](source)),0
    finally:
        if extended is not None:extended.Dispose()
        source.Dispose();boundary.Dispose()
