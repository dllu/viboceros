"""Repeated edits and distance-point input through the public native getter."""
try:
    from . import soft_edit_curve_probe as base
except (ImportError,ValueError):
    import soft_edit_curve_probe as base


def mapped(op):
    return dict((k,v)for k,v in dict(op,op='soft_edit_curve_command').items()if k not in('moves','distance_point'))


def validate(op):
    if not isinstance(op,dict)or op.get('op')!='soft_edit_curve_sequence':raise ValueError('invalid soft edit sequence')
    base.validate(mapped(op))
    moves=op.get('moves',[])
    if not isinstance(moves,list)or len(moves)>8:raise ValueError('invalid soft edit moves')
    for m in moves:
        if not isinstance(m,dict)or set(m)!={'station','offset'}:raise ValueError('invalid soft edit move')
        base.validate(dict(mapped(op),stations=[m['station']],offset=m['offset']))
    if 'distance_point'in op and(type(op['distance_point'])not in(int,float)or not 0<=op['distance_point']<=1):
        raise ValueError('invalid distance point')


def run(op,host,iterations,tolerance):
    validate(op)
    R=host['Rhino'];rhino=base.Forward(R);app=base.Forward(R.RhinoApp);rhino.RhinoApp=app
    state={}
    def token(p):return 'w'+','.join('%.17g'%x for x in p)
    def execute(macro,echo):
        if not macro.startswith('_SoftEditCrv '):return R.RhinoApp.RunScript(macro,echo)
        curve=list(R.RhinoDoc.ActiveDoc.Objects)[0].Geometry
        words=macro.split();points=[]
        for m in op.get('moves',[]):
            p=host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(m['station'])))
            q=[x+y for x,y in zip(p,m['offset'])];points.extend([p,q]);words[-1:-1]=[token(p),token(q)]
        pick=None
        if 'distance_point'in op:
            pick=host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(op['distance_point'])))
            words[words.index('_Distance')+1]=token(pick)
        macro=' '.join(words);state.update(macro=macro,additional_points=points,distance_pick=pick)
        return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;local=dict(host);local['Rhino']=rhino
    value,elapsed=base.run(mapped(op),local,iterations,tolerance)
    value.update(state);value['recipe']=op
    value['all_points']=[value['reference'],value['target']]+state['additional_points']
    return value,elapsed
