"""Tightly constrained public batch length inversions on owned NURBS curves."""
import math,re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def finite(value):return not(math.isnan(value)or math.isinf(value))


def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','curve','fractions'}or op['op']!='arc_length_batch'or not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid arc-length batch recipe')
    f=op['fractions'];c=op['curve']
    if not isinstance(f,list)or len(f)>4096 or any(type(v)not in(int,float)or not 0<=v<=1 or not finite(v)for v in f):raise ValueError('invalid normalized arc-length fractions')
    if not isinstance(c,dict)or not {'degree','control_points','knots'}<=set(c)or set(c)-{'degree','control_points','knots','domain'}:raise ValueError('invalid batch curve definition')
    if type(c['degree'])is not int or not 1<=c['degree']<=16 or not isinstance(c['control_points'],list)or not c['degree']+1<=len(c['control_points'])<=256 or not isinstance(c['knots'],list)or len(c['knots'])!=len(c['control_points'])+c['degree']+1:raise ValueError('invalid batch curve dimensions')
    validate_source(dict(op='geometry_validity',id=op['id'],source=dict(type='nurbs',**c)))


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
        return host['_measure'](iterations,compute)
    finally:curve.Dispose()
