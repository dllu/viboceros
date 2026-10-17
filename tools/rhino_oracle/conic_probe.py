"""Authored public Conic command getters and exact output curves."""
import math,re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source

def validate(op):
    if not isinstance(op,dict)or op.get('op')!='conic_command'or set(op)-{'op','id','actions','curves','construction_plane','cancel_tail','setup'}or not {'id','actions'}<=set(op):raise ValueError('invalid conic recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid conic id')
    def finite(v):return type(v)in(int,float)and not(math.isnan(v)or math.isinf(v))and abs(v)<=1e6
    def point(p):return isinstance(p,list)and len(p)==3 and all(finite(v)for v in p)
    curves=op.get('curves',[])
    if not isinstance(curves,list)or len(curves)>2:raise ValueError('invalid conic curves')
    for curve in curves:
        validate_source(dict(op='geometry_validity',id=op['id'],source=curve))
        if curve.get('type')not in('line','nurbs','arc','circle','polyline','polycurve'):raise ValueError('invalid conic reference')
    if not isinstance(op['actions'],list)or not 1<=len(op['actions'])<=24:raise ValueError('invalid conic actions')
    for action in op['actions']:
        if isinstance(action,str):
            if action not in('Apex','Vertical','Tangent','Perpendicular','Enter','SubCrv','FullCurve'):raise ValueError('invalid conic option')
        elif isinstance(action,dict):
            if set(action)!={'curve'}or type(action['curve'])is not int or not 0<=action['curve']<len(curves):raise ValueError('invalid conic curve pick')
        elif not(point(action)or finite(action)):raise ValueError('invalid conic numeric input')
    if 'construction_plane'in op and(not isinstance(op['construction_plane'],list)or len(op['construction_plane'])!=3 or not all(point(p)for p in op['construction_plane'])):raise ValueError('invalid conic frame')
    if type(op.get('cancel_tail',False))is not bool:raise ValueError('invalid conic flag')
    if 'setup'in op:validate(dict(op='conic_command',id=op['id'],actions=op['setup']))

def run(op,host,iterations,tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    return run_primitive(op,host,iterations,tolerance,'Conic')
