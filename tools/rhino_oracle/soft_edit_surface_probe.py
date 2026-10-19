"""Public SoftEditSrf commands on independently constructed surface sources."""
import math
try:
    from . import insert_kink_probe as base
    from .soft_edit_curve_probe import Forward
except (ImportError,ValueError):
    import insert_kink_probe as base
    from soft_edit_curve_probe import Forward


def mapped(op):
    return dict((k,v)for k,v in dict(op,op='insert_kink_command').items()
        if k not in('distance','offset','fix_edges','copy','constraint','cancel_after'))


def validate(op):
    if not isinstance(op,dict)or op.get('op')!='soft_edit_surface_command':raise ValueError('invalid surface soft edit')
    base.validate(mapped(op))
    if op['family']not in('surface','surface_rational','trimmed_rectangle','trimmed_hole')or len(op['stations'])!=1:
        raise ValueError('one single-face surface and one base pick required')
    if any(k in op for k in('gauge','elevate','domain','reverse','subcurve','face','numeric','direction','symmetrical','midpoints','refine')):
        raise ValueError('unsupported surface source option')
    d=op.get('distance')
    if not isinstance(d,list)or len(d)!=2 or any(type(x)not in(int,float)or not 0<x<1e100 for x in d):raise ValueError('invalid UV distances')
    v=op.get('offset')
    if not isinstance(v,list)or len(v)!=3 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in v):raise ValueError('invalid offset')
    if any(type(op.get(k,False))is not bool for k in('copy','fix_edges','cancel_after')):raise ValueError('invalid soft edit flag')
    if op.get('constraint','None')not in('None','SrfNormal','UTangent','VTangent'):raise ValueError('invalid direction constraint')


def run(op,host,iterations,tolerance):
    validate(op)
    import join_probe
    R=host['Rhino'];rhino=Forward(R);app=Forward(R.RhinoApp);rhino.RhinoApp=app
    state={}
    def execute(macro,echo):
        if not macro.startswith('_InsertKink '):return R.RhinoApp.RunScript(macro,echo)
        words=macro.split();first=next(i for i,w in enumerate(words)if w.startswith('w'))
        reference=[float(x)for x in words[first][1:].split(',')];target=[x+y for x,y in zip(reference,op['offset'])]
        prefix=[w for w in words[1:first]if not w.startswith(('_Direction=','_Symmetrical=','_Midpoints='))]
        words=['_SoftEditSrf']+prefix+[words[first],
            '_U_Distance','%.17g'%op['distance'][0],'_V_Distance','%.17g'%op['distance'][1],
            '_FixEdges='+('Yes'if op.get('fix_edges')else'No'),'_Copy='+('Yes'if op.get('copy')else'No'),
            '_DirConstraint=_'+op.get('constraint','None')]
        if not op.get('cancel')or op.get('cancel_after'):words.append('w'+','.join('%.17g'%x for x in target))
        words.append('!'if op.get('cancel')else'_Enter')
        macro=' '.join(words);state.update(macro=macro,reference=reference,target=target)
        return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;original=join_probe.observe_command
    def observe(event_source,command,*args,**kwargs):
        return original(event_source,'SoftEditSrf'if command=='InsertKink'else command,*args,**kwargs)
    join_probe.observe_command=observe;local=dict(host);local['Rhino']=rhino
    try:
        value,elapsed=base.run(mapped(op),local,iterations,tolerance)
        value.update(state);value['recipe']=op;return value,elapsed
    finally:join_probe.observe_command=original
