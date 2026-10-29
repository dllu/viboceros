"""Ordered public NURBS tangent queries on an owned source."""
import math
import re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    if (not isinstance(op,dict) or set(op) != {'op','id','curve','parameters'}
            or op['op'] != 'nurbs_curve_tangents' or not isinstance(op['id'],str)
            or re.match(r'\A[A-Za-z0-9_.-]{1,100}\Z',op['id']) is None):
        raise ValueError('invalid curve tangent recipe')
    curve,parameters = op['curve'],op['parameters']
    if (not isinstance(curve,dict) or not {'degree','knots','control_points'} <= set(curve)
            or set(curve)-{'degree','knots','control_points','domain'}
            or type(curve['degree']) is not int or not 1 <= curve['degree'] <= 16
            or not isinstance(curve['control_points'],list)
            or not curve['degree']+1 <= len(curve['control_points']) <= 512
            or not isinstance(curve['knots'],list)
            or len(curve['knots']) != len(curve['control_points'])+curve['degree']+1
            or not isinstance(parameters,list) or not 1 <= len(parameters) <= 4096):
        raise ValueError('invalid curve tangent dimensions or query count')
    source=dict(curve)
    if source.get('domain') is None: source.pop('domain',None)
    validate_source(dict(op='geometry_validity',id=op['id'],source=dict(type='nurbs',**source)))
    domain=curve.get('domain')
    if domain is None: domain=[curve['knots'][curve['degree']],curve['knots'][len(curve['control_points'])]]
    finite=lambda x:type(x) in (int,float) and not math.isnan(x) and not math.isinf(x)
    if (not isinstance(domain,list) or len(domain)!=2 or not all(finite(t)for t in domain)
            or domain[0]>=domain[1]):raise ValueError('invalid curve tangent domain')
    start,end = domain
    if any(type(t) not in (int,float) or math.isnan(t) or math.isinf(t)
           or t < start or t > end for t in parameters):
        raise ValueError('curve tangent parameters are outside the active domain')


def run(op,host,iterations,tolerance):
    validate(op)
    source = host['_nurbs_curve_from_definition'](op['curve'])
    try:
        if not source.IsValid: raise ValueError('invalid native tangent source')
        before = host['_nurbs_curve_definition'](source)
        def query():
            values = []
            for parameter in op['parameters']:
                direction = source.TangentAt(parameter)
                values.append(None if direction.IsZero else host['_xyz'](direction))
            return values
        tangents,elapsed = host['_measure'](iterations,query)
        after = host['_nurbs_curve_definition'](source)
        if before != after: raise ValueError('tangent query changed source')
        return dict(before=before,after=after,parameters=op['parameters'],tangents=tangents),elapsed
    finally:
        source.Dispose()
