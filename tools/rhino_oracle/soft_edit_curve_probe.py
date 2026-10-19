"""Public SoftEditCrv commands over independently constructed curve sources."""
import math
try:
    from . import insert_kink_probe as base
except (ImportError, ValueError):
    import insert_kink_probe as base


def plain(op):
    return dict((k,v) for k,v in dict(op,op='insert_kink_command').items()
                if k not in ('distance','offset','fix_ends','copy','dense','cancel_after'))


def validate(op):
    if not isinstance(op,dict) or op.get('op')!='soft_edit_curve_command':
        raise ValueError('invalid soft edit recipe')
    base.validate(plain(op))
    if op['family'] not in ('cubic','rational','periodic','circle','ellipse','line','polyline','polycurve') or len(op['stations'])!=1:
        raise ValueError('soft edit needs one curve and one reference pick')
    d=op.get('distance')
    if type(d) not in (int,float) or not 0<d<1e100:
        raise ValueError('invalid falloff distance')
    v=op.get('offset')
    if not isinstance(v,list) or len(v)!=3 or any(type(x)not in(int,float) or math.isnan(x) or math.isinf(x) for x in v):
        raise ValueError('invalid displacement')
    if any(type(op.get(k,False))is not bool for k in ('copy','fix_ends','dense','cancel_after')):
        raise ValueError('invalid soft edit flag')
    if any(k in op for k in ('direction','symmetrical','midpoints','face','v_fraction','numeric','subcurve')):
        raise ValueError('unsupported soft edit recipe')


class Forward:
    def __init__(self,target):self.target=target
    def __getattr__(self,name):return getattr(self.target,name)


def run(op,host,iterations,tolerance):
    validate(op)
    import join_probe
    R=host['Rhino'];app=Forward(R.RhinoApp);rhino=Forward(R);rhino.RhinoApp=app
    state={}
    def execute(macro,echo):
        if not macro.startswith('_InsertKink '):return R.RhinoApp.RunScript(macro,echo)
        words=macro.split();first=next(i for i,w in enumerate(words)if w.startswith('w'))
        reference=[float(x)for x in words[first][1:].split(',')]
        target=[x+y for x,y in zip(reference,op['offset'])]
        options=['_Distance','%.17g'%op['distance'],'_FixEnds='+('Yes'if op.get('fix_ends')else'No'),'_Copy='+('Yes'if op.get('copy')else'No')]
        words[0]='_SoftEditCrv'
        words[first+1:first+1]=options
        if not op.get('cancel') or op.get('cancel_after'):words.insert(-1,'w'+','.join('%.17g'%x for x in target))
        macro=' '.join(words);state.update(macro=macro,reference=reference,target=target)
        return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute
    original=join_probe.observe_command
    def observe(event_source,command,*args,**kwargs):
        return original(event_source,'SoftEditCrv'if command=='InsertKink'else command,*args,**kwargs)
    join_probe.observe_command=observe
    local=dict(host);local['Rhino']=rhino
    factory=host['_nurbs_curve_from_definition']
    def source(definition):
        if op.get('dense'):
            definition=dict(degree=3,knots=[0.]*4+[float(i)for i in range(1,6)]+[6.]*4,
                control_points=[dict(point=[float(i),float((i%3)-1),.2*float(i%2)],weight=1.+.3*(i%3)if op['family']=='rational'else 1.)for i in range(9)])
        return factory(definition)
    local['_nurbs_curve_from_definition']=source
    try:
        value,elapsed=base.run(plain(op),local,iterations,tolerance)
        value.update(state);value['recipe']=op
        return value,elapsed
    finally:join_probe.observe_command=original
