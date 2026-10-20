"""Repeated public closed-surface edits, failed copies and unfinished offsets."""
try:
    from . import soft_edit_surface_seams_probe as base
    from .soft_edit_curve_probe import Forward
except (ImportError,ValueError):
    import soft_edit_surface_seams_probe as base
    from soft_edit_curve_probe import Forward

def mapped(op):return dict((k,v)for k,v in dict(op,op='soft_edit_surface_seams').items()if k not in('moves','unfinished'))
def validate(op):
    if not isinstance(op,dict)or op.get('op')!='soft_edit_surface_closed_sequence':raise ValueError('invalid closed sequence')
    base.validate(mapped(op));moves=op.get('moves')
    if not isinstance(moves,list)or not 1<=len(moves)<=8:raise ValueError('invalid repeated moves')
    for move in moves:
        if not isinstance(move,dict)or set(move)-{'uv','offset','copy'}or not {'uv','offset'}<=set(move):raise ValueError('invalid repeated move')
        uv=move['uv']
        if not isinstance(uv,list)or len(uv)!=2:raise ValueError('invalid UV pick')
        base.validate(dict(mapped(op),stations=[uv[0]],v_fraction=uv[1],offset=move['offset'],copy=move.get('copy',False)))
    if 'unfinished'in op:
        uv=op['unfinished']
        if not isinstance(uv,list)or len(uv)!=2:raise ValueError('invalid unfinished pick')
        base.validate(dict(mapped(op),stations=[uv[0]],v_fraction=uv[1]))

def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];rhino=Forward(R);app=Forward(R.RhinoApp);rhino.RhinoApp=app;state={}
    def token(p):return 'w'+','.join('%.17g'%x for x in p)
    def execute(macro,echo):
        if not macro.startswith('_SoftEditSrf '):return R.RhinoApp.RunScript(macro,echo)
        surface=list(R.RhinoDoc.ActiveDoc.Objects)[0].Geometry.Faces[0];words=macro.split();points=[]
        def point(uv):return host['_xyz'](surface.PointAt(surface.Domain(0).ParameterAt(uv[0]),surface.Domain(1).ParameterAt(uv[1])))
        for move in op['moves']:
            p=point(move['uv']);q=[x+y for x,y in zip(p,move['offset'])];points.extend([p,q]);extra=[]
            if 'copy'in move:extra.append('_Copy='+('Yes'if move['copy']else'No'))
            words[-1:-1]=extra+[token(p),token(q)]
        if 'unfinished'in op:p=point(op['unfinished']);points.append(p);words[-1:-1]=[token(p)]
        macro=' '.join(words);state.update(macro=macro,additional_points=points);return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;local=dict(host);local['Rhino']=rhino;value,elapsed=base.run(mapped(op),local,iterations,tolerance);value.update(state);value['recipe']=op;value['all_points']=[value['reference'],value['target']]+state['additional_points'];return value,elapsed
