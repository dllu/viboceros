"""Strict finite NURBS definitions shared by owned construction probes."""
import math


def validate_curve_definition(curve):
    if (not isinstance(curve,dict) or set(curve) != {'degree','knots','control_points'}
            or type(curve['degree']) is not int or not 1 <= curve['degree'] <= 64
            or not isinstance(curve['control_points'],list) or not curve['degree'] < len(curve['control_points']) <= 10000
            or not isinstance(curve['knots'],list) or len(curve['knots']) != len(curve['control_points'])+curve['degree']+1):
        raise ValueError('invalid NURBS curve dimensions')
    for cp in curve['control_points']:
        if (not isinstance(cp,dict) or set(cp) != {'point','weight'} or not isinstance(cp['point'],list) or len(cp['point']) != 3):
            raise ValueError('invalid NURBS control')
        values=cp['point']+[cp['weight']]
        if any(type(x) not in (int,float) or math.isnan(x) or math.isinf(x) for x in values) or cp['weight'] == 0:
            raise ValueError('invalid NURBS coordinates or weight')
    if (any(type(x) not in (int,float) or math.isnan(x) or math.isinf(x) for x in curve['knots'])
            or any(a>b for a,b in zip(curve['knots'],curve['knots'][1:]))):
        raise ValueError('invalid NURBS knots')
