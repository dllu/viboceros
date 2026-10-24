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
