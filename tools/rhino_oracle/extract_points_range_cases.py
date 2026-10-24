"""Owned temporary curve references, with no executable request payloads."""
CASES = {}
for kind in ['line', 'rational', 'polyline', 'circle', 'polycurve', 'periodic']:
    for output, layer in [('Points', 'Input'), ('PointCloud', 'Current')]:
        CASES['range_'+kind+'_'+output.lower()] = dict(
            sources=[dict(kind=kind, mode=None, grips=[])], whole=[], order=[0],
            output=output, layer=layer, ranges=[[0, .2, .7]])
for output in ['Points', 'PointCloud']:
    base = CASES['range_circle_'+output.lower()]
    CASES['range_circle_wrap_'+output.lower()] = dict(base, ranges=[[0,.8,.2]])
    base = CASES['range_rational_'+output.lower()]
    CASES['range_other_layer_'+output.lower()] = dict(base,
        layer='Current' if output=='Points' else 'Input')
    CASES['range_with_whole_'+output.lower()] = dict(base,
        sources=base['sources']+[dict(kind='line', mode=None, grips=[])],
        whole=[1], order=[0,1])
    CASES['range_twice_'+output.lower()] = dict(base,
        ranges=[[0,.2,.4],[0,.6,.8]])

GRIPS = {}
for output in ['Points','PointCloud']:
    for layer in ['Input','Current']:
        GRIPS['range_grips_'+output.lower()+'_'+layer.lower()] = dict(
            CASES['range_rational_'+output.lower()],
            sources=[dict(kind='rational',mode='control',grips=[])],
            layer=layer, range_grips=True)

OWNERS = {'range_whole_owner_'+output.lower(): dict(
    CASES['range_rational_'+output.lower()], whole=[0])
    for output in ['Points','PointCloud']}

EDGE_DISCOVERY={
    'range_edge_'+kind+'_discovery':dict(
        sources=[dict(kind=kind,mode=None,grips=[]),dict(kind='line',mode=None,grips=[])],
        whole=[1],order=[0,1],output='Points',layer='Input',edge_ranges=[[0,0,.2,.7]])
    for kind in ['brep_box','brep_surface']}

EDGES={}
for kind,index,start,end in [
        ('brep_box',0,.2,.7),('brep_surface',0,.2,.7),('brep_surface',2,.7,.2),
        ('brep_trimmed',0,.2,.7),('brep_trimmed',2,.2,.7),
        ('brep_cylinder',0,.2,.7),('brep_cylinder',0,.8,.2),
        ('brep_periodic_joined',0,.2,.7)]:
    for output,layer in [('Points','Input'),('PointCloud','Current')]:
        name='range_edge_%s_%d_%s_%s'%(kind,index,'wrap' if start>end else 'forward',output.lower())
        EDGES[name]=dict(sources=[dict(kind=kind,mode=None,grips=[]),dict(kind='line',mode=None,grips=[])],
            whole=[1],order=[0,1],output=output,layer=layer,edge_ranges=[[0,index,start,end]],range_whole_first=True)

EDGE_FORMS={}
for kind in ['brep_cylinder','brep_nurbs_cylinder']:
    for start,end in [(.2,.7),(.8,.2)]:
        for output,layer in [('Points','Current'),('PointCloud','Input')]:
            EDGE_FORMS['range_form_%s_%s_%s'%(kind,'wrap' if start>end else 'forward',output.lower())]=dict(
                sources=[dict(kind=kind,mode=None,grips=[])],whole=[],order=[0],output=output,layer=layer,
                edge_ranges=[[0,0,start,end]],range_whole_first=True)

EDGE_NUMERIC={}
for kind in ['brep_cylinder','brep_nurbs_cylinder']:
    for start,end,length,output,layer in [(.8,.2,.4,'Points','Input'),(.2,.7,.3,'PointCloud','Current')]:
        EDGE_NUMERIC['range_numeric_%s_%s'%(kind,output.lower())]=dict(
            sources=[dict(kind=kind,mode=None,grips=[])],whole=[],order=[0],output=output,layer=layer,
            edge_ranges=[[0,0,start,end]],range_whole_first=True,edge_length=length)

EDGE_NUMERIC_VERIFIED={name.replace('range_numeric_','range_numeric_verified_'):dict(case,edge_ranges=[list(r) for r in case['edge_ranges']])
    for name,case in EDGE_NUMERIC.items()}
for case in EDGE_NUMERIC_VERIFIED.values():
    if case['output']=='PointCloud':case['edge_ranges'][0][3]=.8
