"""Stretch selected control/edit grips using the shared owned grip fixtures."""
try:
    from . import grip_transform_probe as base
    from .stretch_geometry_probe import Forward
except (ImportError,ValueError):
    import grip_transform_probe as base
    from stretch_geometry_probe import Forward


def mapped(op):
    return dict(op='grip_transform',id=op['id'],source=op['source'],selected=op['selected'],parent=op.get('parent',False),point=op.get('point',False),command='move',off=False,inputs='auto')

def validate(op):
    if not isinstance(op,dict)or set(op)-{'parent','point','targets'}!={'op','id','source','selected','copy','rigid'}or op['op']!='stretch_grips' or any(type(op[k])is not bool for k in ('copy','rigid')):raise ValueError('invalid Stretch grip recipe')
    base.validate(mapped(op))
    if 'targets'in op and (not op['copy']or not isinstance(op['targets'],list)or not 1<=len(op['targets'])<=8 or any(type(v)not in(int,float)or not -10<=v<=10 for v in op['targets'])):raise ValueError('invalid repeated grip factors')


def run(op,host,iterations,tolerance):
    validate(op)
    import join_probe
    R=host['Rhino'];app=Forward(R.RhinoApp);rhino=Forward(R);rhino.RhinoApp=app;state={}
    def execute(macro,echo):
        if not macro.startswith('_Move '):return R.RhinoApp.RunScript(macro,echo)
        macro='_Stretch w-1,0,0 w1,0,0 _Copy='+('Yes'if op['copy']else'No')+' _Rigid='+('Yes'if op['rigid']else'No')+' 2'+''.join(' '+str(v)for v in op.get('targets',[]))+(' _Enter'if op['copy']else'');state['macro']=macro;return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;original=join_probe.observe_command
    def observe(event_source,name,*args,**kw):return original(event_source,'Stretch'if name=='Move'else name,*args,**kw)
    join_probe.observe_command=observe;local=dict(host);local['Rhino']=rhino
    try:
        value,elapsed=base.run(mapped(op),local);value['recipe']=op;value.update(state);return value,elapsed
    finally:join_probe.observe_command=original
