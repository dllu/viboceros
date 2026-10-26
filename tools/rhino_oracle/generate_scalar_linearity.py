"""Separate geometric straightness from degree in scalar inverse stopping."""
import copy
import json
from .capture_family import HERE, capture


def elevate(source, target):
    degree = source['degree']
    net = [[x*cp['weight'] for x in cp['point']] + [cp['weight']]
           for cp in source['control_points']]
    if len(net) != degree+1:
        raise ValueError('single Bezier required')
    while degree < target:
        order = degree+1
        net = [net[0]] + [[(i/order)*a+(1.-i/order)*b for a,b in zip(net[i-1],net[i])]
                         for i in range(1,order)] + [net[-1]]
        degree += 1
    return dict(degree=degree,knots=[0.]*(degree+1)+[1.]*(degree+1),
        control_points=[dict(point=[x/p[3] for x in p[:3]],weight=p[3]) for p in net])


def request():
    query = json.loads((HERE/'fixtures/loft_scalar_clock.json').read_text())
    sources = {o['id']:o['curves'][0] for o in query['operations']}
    curves = []
    cubic = sources['loft-scalar-clock-cubic-0.1']
    for epsilon in [0.,1e-8,1e-4,.1,1.]:
        curve = copy.deepcopy(cubic)
        for cp in curve['control_points']:
            cp['point'][1] *= epsilon
        curves.append(('cubic-y-'+str(epsilon),curve))
    line = sources['loft-scalar-clock-rational_line']
    for degree in [1,2,3,5]:
        curves.append(('line-degree-'+str(degree),elevate(line,degree)))
    query['operations'] = [dict(op='curve_length_parameters',id='scalar-linearity-'+name,
        curve=curve,fractional_tolerance=.01,fractions=[1./64.,.125,.5,.875,63./64.])
        for name,curve in curves]
    return query


if __name__=='__main__':
    capture('scalar_linearity',request(),['generate_scalar_linearity.py',
        'fixtures/loft_scalar_clock.json','loft_scalar_clock_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleScalarLinearity20261025')
