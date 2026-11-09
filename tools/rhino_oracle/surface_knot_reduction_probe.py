"""Public surface knot-list ranges on private, owned NURBS duplicates."""
import math


def validate(op):
    required={'op','id','surface'}
    if (not isinstance(op,dict) or not required.issubset(op)
            or set(op)-required-{'direction','minimum_multiplicity','maximum_multiplicity','absolute_tolerance'}
            or op['op']!='surface_knot_reduction' or not isinstance(op['id'],str)
            or not isinstance(op['surface'],dict) or op.get('direction','both')not in ['u','v','both']):
        raise ValueError('invalid surface knot reduction operation')
    minimum=op.get('minimum_multiplicity',1);maximum=op.get('maximum_multiplicity')
    if (type(minimum)is not int or minimum<0 or maximum is not None and(type(maximum)is not int or maximum<=minimum)):
        raise ValueError('invalid surface knot reduction range')
    if op.get('absolute_tolerance')is not None:
        value=op['absolute_tolerance']
        if type(value)not in(int,float)or math.isnan(value)or math.isinf(value)or value<0.:
            raise ValueError('invalid surface knot reduction tolerance')


def run(op,host,iterations,tolerance):
    validate(op)
    source=host['_nurbs_surface_from_definition'](op['surface'])
    definition=host['_nurbs_surface_definition']
    try:
        direction=op.get('direction','both');minimum=op.get('minimum_multiplicity',1)
        maximum=op.get('maximum_multiplicity')
        if maximum is None:maximum=max(source.Degree(0),source.Degree(1))+1
        if maximum<=minimum:raise ValueError('invalid default surface knot reduction range')
        limit=op.get('absolute_tolerance')
        if limit is None:limit=tolerance['absolute']
        before=definition(source)
        def compute():
            candidate=source.Duplicate()
            if candidate is None:raise ValueError('surface duplication failed')
            try:
                removed=0
                if direction in ['u','both']:removed+=candidate.KnotsU.RemoveMultipleKnots(minimum,maximum,limit)
                if direction in ['v','both']:removed+=candidate.KnotsV.RemoveMultipleKnots(minimum,maximum,limit)
                if not candidate.IsValid:raise ValueError('invalid surface knot reduction result')
                after=definition(source)
                if after!=before:raise ValueError('surface knot reduction mutated its input')
                return dict(removed=int(removed),surface=definition(candidate),before=before,after=after,
                    samples=[host['_xyz'](candidate.PointAt(candidate.Domain(0).ParameterAt(i/8.),candidate.Domain(1).ParameterAt(j/8.)))
                             for i in range(9)for j in range(9)])
            finally:candidate.Dispose()
        return host['_measure'](iterations,compute)
    finally:source.Dispose()
