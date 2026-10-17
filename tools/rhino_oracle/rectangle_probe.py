"""Completed public Rectangle getters with exact curve structure and history."""
import math,re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    if not isinstance(op,dict)or op.get('op')!='rectangle'or set(op)-{'op','id','actions','construction_plane','curve','cancel_tail','preselect','setup'}or not {'id','actions'}<=set(op):raise ValueError('invalid rectangle recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,100}\Z',op['id'])is None:raise ValueError('invalid rectangle id')
    def finite(n):return type(n)in(int,float)and not(math.isnan(n)or math.isinf(n))and abs(n)<=1e6
    def point(p):return isinstance(p,list)and len(p)==3 and all(finite(n)for n in p)
    if not isinstance(op['actions'],list)or not 1<=len(op['actions'])<=24:raise ValueError('invalid rectangle actions')
    for item in op['actions']:
        if isinstance(item,str):
            if len(item)>64:raise ValueError('rectangle option too long')
            if item not in('3Point','Center','Vertical','EdgeMidpoint','AroundCurve','Enter','Rounded','Corner','Arc','Conic','Corner=Arc','Corner=Conic','Rounded=Yes','Rounded=No')and not re.match(r'\A(?:Rho|Radius)=[0-9]+(?:\.[0-9]+)?\Z',item):raise ValueError('invalid rectangle option')
        elif not(point(item)or finite(item)):raise ValueError('invalid rectangle action')
    if 'construction_plane'in op and(not isinstance(op['construction_plane'],list)or len(op['construction_plane'])!=3 or not all(point(p)for p in op['construction_plane'])):raise ValueError('invalid rectangle frame')
    if 'curve'in op:
        validate_source(dict(op='geometry_validity',id=op['id'],source=op['curve']))
        if op['curve'].get('type')not in('line','nurbs','arc','circle','polyline','polycurve'):raise ValueError('invalid rectangle guide')
    if 'setup'in op:validate(dict(op='rectangle',id=op['id'],actions=op['setup']))
    for key in('cancel_tail','preselect'):
        if type(op.get(key,False))is not bool:raise ValueError('invalid rectangle flag')


def run(op,host,iterations,tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    return run_primitive(op,host,iterations,tolerance,'Rectangle')
