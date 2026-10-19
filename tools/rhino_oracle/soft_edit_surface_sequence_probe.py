"""Repeated public surface getter pairs, including changes to Copy between pairs."""
try:
    from . import soft_edit_surface_probe as base
except (ImportError,ValueError):
    import soft_edit_surface_probe as base

def mapped(op):return dict((k,v)for k,v in dict(op,op='soft_edit_surface_command').items()if k not in('moves',))
def validate(op):
    if not isinstance(op,dict)or op.get('op')!='soft_edit_surface_sequence':raise ValueError('invalid surface sequence')
    base.validate(mapped(op))
    moves=op.get('moves')
    if not isinstance(moves,list)or not 1<=len(moves)<=8:raise ValueError('invalid surface moves')
    for move in moves:
        if not isinstance(move,dict)or set(move)-{'uv','offset','copy'}or not {'uv','offset'}<=set(move):raise ValueError('invalid surface move')
        uv=move['uv']
        if not isinstance(uv,list)or len(uv)!=2:raise ValueError('invalid UV pick')
        base.validate(dict(mapped(op),stations=[uv[0]],v_fraction=uv[1],offset=move['offset'],copy=move.get('copy',False)))

def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];rhino=base.Forward(R);app=base.Forward(R.RhinoApp);rhino.RhinoApp=app;state={}
    def token(p):return 'w'+','.join('%.17g'%x for x in p)
    def execute(macro,echo):
        if not macro.startswith('_SoftEditSrf '):return R.RhinoApp.RunScript(macro,echo)
        geometry=list(R.RhinoDoc.ActiveDoc.Objects)[0].Geometry
        surface=geometry.Faces[0]if isinstance(geometry,R.Geometry.Brep)else geometry
        words=macro.split();points=[]
        for move in op['moves']:
            uv=move['uv'];p=host['_xyz'](surface.PointAt(surface.Domain(0).ParameterAt(uv[0]),surface.Domain(1).ParameterAt(uv[1])))
            q=[x+y for x,y in zip(p,move['offset'])];points.extend([p,q]);insert=[]
            if 'copy'in move:insert.append('_Copy='+('Yes'if move['copy']else'No'))
            insert.extend([token(p),token(q)]);words[-1:-1]=insert
        macro=' '.join(words);state.update(macro=macro,additional_points=points)
        return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;local=dict(host);local['Rhino']=rhino
    value,elapsed=base.run(mapped(op),local,iterations,tolerance);value.update(state);value['recipe']=op;value['all_points']=[value['reference'],value['target']]+state['additional_points'];return value,elapsed
