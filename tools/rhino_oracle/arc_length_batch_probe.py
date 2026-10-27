"""Tightly constrained public batch length inversions on owned NURBS curves."""
import math,re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def finite(value):return not(math.isnan(value)or math.isinf(value))


def validate(op):
    required={'op','id','curve','fractions'}
    if not isinstance(op,dict)or not required<=set(op)or set(op)-required-{'inspect_numerical_batch'}or op['op']!='arc_length_batch'or not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid arc-length batch recipe')
    f=op['fractions'];c=op['curve']
    if not isinstance(f,list)or len(f)>4096 or any(type(v)not in(int,float)or not 0<=v<=1 or not finite(v)for v in f):raise ValueError('invalid normalized arc-length fractions')
    if not isinstance(c,dict)or not {'degree','control_points','knots'}<=set(c)or set(c)-{'degree','control_points','knots','domain'}:raise ValueError('invalid batch curve definition')
    if type(c['degree'])is not int or not 1<=c['degree']<=16 or not isinstance(c['control_points'],list)or not c['degree']+1<=len(c['control_points'])<=256 or not isinstance(c['knots'],list)or len(c['knots'])!=len(c['control_points'])+c['degree']+1:raise ValueError('invalid batch curve dimensions')
    validate_source(dict(op='geometry_validity',id=op['id'],source=dict(type='nurbs',**c)))
    if 'inspect_numerical_batch' in op:
        if f!=sorted(f):raise ValueError('numerical inspection fractions must be nondecreasing')
        scenarios=op['inspect_numerical_batch']
        if not isinstance(scenarios,list)or not 1<=len(scenarios)<=16:raise ValueError('invalid numerical batch scenarios')
        labels=set()
        for scenario in scenarios:
            if not isinstance(scenario,dict)or set(scenario)!={'id','fractional_tolerance','history'}:raise ValueError('invalid numerical batch scenario')
            label=scenario['id']
            if not isinstance(label,str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',label)is None or label in labels:raise ValueError('invalid numerical batch scenario identity')
            labels.add(label)
            _accuracy(scenario['fractional_tolerance'])
            history=scenario['history']
            if not isinstance(history,list)or len(history)>8:raise ValueError('invalid numerical batch history')
            for query in history:
                if not isinstance(query,dict)or set(query)!={'query','fractional_tolerance'}or query['query']not in('length','scalar','batch'):raise ValueError('invalid numerical batch history query')
                _accuracy(query['fractional_tolerance'])


def _accuracy(value):
    if type(value)not in(int,float)or not 0<value<1:raise ValueError('invalid numerical batch accuracy')


def _numerical_batches(op,host,tolerance):
    """Use a fresh curve per scenario; length reporting must follow inversion."""
    fractions=op['fractions']
    inputs=host['System'].Array[host['System'].Double](fractions)
    records=[]
    for scenario in op['inspect_numerical_batch']:
        curve=host['_object_source'](dict(type='nurbs',**op['curve']),tolerance)
        try:
            before=host['_nurbs_curve_definition'](curve)
            def batch(accuracy):
                values=curve.NormalizedLengthParameters(inputs,0.,accuracy)
                if values is None or len(values)!=len(fractions):raise ValueError('numerical batch inversion failed')
                return [float(v)for v in values]
            for query in scenario['history']:
                accuracy=query['fractional_tolerance']
                if query['query']=='length':
                    length=curve.GetLength(accuracy)
                    if not finite(length)or length<=0:raise ValueError('history length query failed')
                elif query['query']=='scalar':
                    success,parameter=curve.NormalizedLengthParameter(.5,accuracy)
                    if not success or not finite(parameter):raise ValueError('history scalar query failed')
                else:batch(accuracy)
            parameters=batch(scenario['fractional_tolerance'])
            records.append(dict(id=scenario['id'],fractions=fractions,parameters=parameters,
                points=[host['_xyz'](curve.PointAt(t))for t in parameters],before=before,
                after=host['_nurbs_curve_definition'](curve)))
        finally:curve.Dispose()
    return records


def run(op,host,iterations,tolerance):
    validate(op)
    curve=host['_object_source'](dict(type='nurbs',**op['curve']),tolerance)
    try:
        length=curve.GetLength(1e-12)
        if not finite(length)or length<=0:raise ValueError('invalid batch curve length')
        fractions=sorted(set(op['fractions']))
        absolute=max(64*2.220446049250313e-16*length,min(tolerance['absolute']*1e-6,tolerance['relative']*length),2.2250738585072014e-308)
        def compute():
            if fractions:
                inputs=host['System'].Array[host['System'].Double](fractions)
                values=curve.NormalizedLengthParameters(inputs,absolute,min(tolerance['relative'],1e-12))
                if values is None or len(values)!=len(fractions):raise ValueError('public batch inversion failed')
                by_fraction=dict(zip(fractions,values));parameters=[by_fraction[f]for f in op['fractions']]
            else:parameters=[]
            return dict(parameters=parameters,points=[host['_xyz'](curve.PointAt(t))for t in parameters],length=length,domain=[curve.Domain.T0,curve.Domain.T1])
        value,elapsed=host['_measure'](iterations,compute)
        if 'inspect_numerical_batch' in op:
            value['numerical_batches']=_numerical_batches(op,host,tolerance)
        return value,elapsed
    finally:curve.Dispose()
