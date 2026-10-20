"""Public SDK Stretch fitting at explicit tolerances and the SDK default."""
import math
try:
    from . import twist_command_probe as source
except (ImportError,ValueError):
    import twist_command_probe as source

def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','shape','axis','factor','fit_tolerance'}or op['op']!='stretch_fit_sdk' or op['shape']not in source.SHAPES or op['axis']not in('Z','Spatial','Skew'):raise ValueError('invalid Stretch fit recipe')
    if not isinstance(op['id'],str)or not op['id'].replace('_','').isalnum():raise ValueError('invalid Stretch fit identity')
    for key in('factor','fit_tolerance'):
        v=op[key]
        if type(v)not in(int,float)or math.isnan(v)or math.isinf(v)or not 0<=v<=10:raise ValueError('invalid Stretch fit scalar')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one Stretch SDK fit required')
    R=host['Rhino'];owned=source.source_geometry(op['shape'],host);saved=R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance
    axis={'Z':([0.,0.,0.],[0.,0.,10.]),'Spatial':([1.,2.,3.],[5.,6.,11.]),'Skew':([1.,2.,3.],[4.,-2.,11.])}[op['axis']]
    a,b=[host['_point'](p)for p in axis];morph=R.Geometry.Morphs.StretchSpaceMorph(a,b,a.DistanceTo(b)*op['factor']);morph.Tolerance=op['fit_tolerance']
    try:
        R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance=1e-6
        before=[source.geometry_record(g,host)for g in owned];status=[bool(morph.Morph(g))for g in owned];after=[source.geometry_record(g,host)for g in owned]
        return dict(source=before,result=after,status=status,tolerance=float(morph.Tolerance)),0
    finally:
        for g in owned:g.Dispose()
        morph.Dispose();R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance=saved
