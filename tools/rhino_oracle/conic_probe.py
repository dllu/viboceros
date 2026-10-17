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
        if curve.get('type')not in('line','nurbs','arc','circle','polyline','polycurve','surface','box_brep','cylinder_brep'):raise ValueError('invalid conic reference')
        kind=curve['type']
        if kind=='polyline'and(not isinstance(curve.get('vertices'),list)or not 2<=len(curve['vertices'])<=4096 or not all(point(p)for p in curve['vertices'])):raise ValueError('invalid conic polyline vertices')
        if kind=='surface':
            counts=[]
            for axis in ('u','v'):
                degree=curve.get('degree_'+axis);count=curve.get('control_point_count_'+axis);knots=curve.get('knots_'+axis)
                if type(degree)is not int or not 1<=degree<=16 or type(count)is not int or not degree<count<=64 or not isinstance(knots,list)or len(knots)!=count+degree+1 or not all(finite(x)for x in knots)or any(a>b for a,b in zip(knots,knots[1:])):raise ValueError('invalid conic surface chart')
                counts.append(count)
            controls=curve.get('control_points')
            if not isinstance(controls,list)or len(controls)!=counts[0]*counts[1]or any(not isinstance(c,dict)or not point(c.get('point'))or not finite(c.get('weight'))or c['weight']<=0 for c in controls):raise ValueError('invalid conic surface controls')
        if kind=='box_brep'and(not point(curve.get('min'))or not point(curve.get('max'))or any(a>=b for a,b in zip(curve['min'],curve['max']))):raise ValueError('invalid conic box')
        if kind=='cylinder_brep'and(not point(curve.get('origin'))or any(not finite(curve.get(k))or curve[k]<=0 for k in ('radius','height'))):raise ValueError('invalid conic cylinder')

    if not isinstance(op['actions'],list)or not 1<=len(op['actions'])<=24:raise ValueError('invalid conic actions')
    for action in op['actions']:
        if isinstance(action,str):
            if action not in('Apex','Vertical','Tangent','Perpendicular','Enter','SubCrv','FullCurve','D','Direction=Locked','Direction=Free'):raise ValueError('invalid conic option')
        elif isinstance(action,dict):
            if set(action)=={'hover'}:
                if not point(action['hover']):raise ValueError('invalid conic hover')
                continue
            if not {'curve'}<=set(action)or set(action)-{'curve','edge','pick'}or type(action['curve'])is not int or not 0<=action['curve']<len(curves):raise ValueError('invalid conic curve pick')
            if 'edge'in action:
                if set(action)!={'curve','edge','pick'}or type(action['edge'])is not int or not 0<=action['edge']<64 or not point(action['pick'])or curves[action['curve']]['type']not in('surface','box_brep','cylinder_brep'):raise ValueError('invalid conic edge pick')
            elif 'pick'in action or curves[action['curve']]['type']in('surface','box_brep','cylinder_brep'):raise ValueError('edge pick required for surface reference')
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
    driver=None
    if any(isinstance(a,dict)and('hover'in a or'edge'in a)for a in op['actions']):
        from conic_getter_input import drive
        driver=lambda guides:drive(op,host,guides)
    return run_primitive(op,host,iterations,tolerance,'Conic',driver)
