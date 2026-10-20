"""Stretch geometry commands reuse owned deformation fixtures and observers."""
try:
    from . import taper_command_probe as base
except (ImportError,ValueError):
    import taper_command_probe as base


def mapped(op):
    return dict(op='taper_geometry_command',id=op['id'],shape=op['shape'],copy=op['copy'],rigid=op['rigid'],flat=False,infinite=False,preserve=False,grouped=op['grouped'],start_distance=1.,end_distance=2.,finish=op.get('finish','Complete'),axis=op.get('axis','Z'),tolerance=op.get('tolerance',1e-6))

def validate(op):
    if not isinstance(op,dict)or op.get('op')!='stretch_geometry_command':raise ValueError('invalid Stretch geometry recipe')
    required={'op','id','shape','copy','rigid','grouped'};values={'factor','length','target'}.intersection(op)
    if len(values)!=1 or set(op)-required-values-{'axis','tolerance','finish','targets','early_factor'}:raise ValueError('invalid Stretch geometry keys')
    base.validate(mapped(op))
    value=op[next(iter(values))]
    if not base.distance_valid(value)or ('target'in op)!=isinstance(value,list):raise ValueError('invalid Stretch target')
    if type(op.get('early_factor',False))is not bool or op.get('early_factor',False)and'factor'not in op:raise ValueError('invalid Stretch early factor')
    if 'targets'in op and (not op['copy']or not isinstance(op['targets'],list)or not 1<=len(op['targets'])<=8 or any(not base.distance_valid(v)for v in op['targets'])):raise ValueError('invalid repeated Stretch inputs')


class Forward:
    def __init__(self,target):self.target=target
    def __getattr__(self,name):return getattr(self.target,name)


def run(op,host,iterations,tolerance):
    validate(op)
    import join_probe
    R=host['Rhino'];app=Forward(R.RhinoApp);rhino=Forward(R);rhino.RhinoApp=app;state={}
    from number_token import number_token
    def value(v):return 'w'+','.join(number_token(x)for x in v)if isinstance(v,list)else number_token(v)
    def execute(macro,echo):
        if not macro.startswith('_Taper '):return R.RhinoApp.RunScript(macro,echo)
        words=['_Stretch']+base.AXES[op.get('axis','Z')].split()
        words+=['_Copy='+('Yes'if op['copy']else'No'),'_Rigid='+('Yes'if op['rigid']else'No')]
        if op.get('early_factor'):
            words = ['_Stretch'] + words[3:] + [words[1], number_token(op['factor']), words[2]]
        else:
            if 'length'in op:words+=['_Length',number_token(op['length'])]
            elif'target'in op:words+=[value(op['target'])]
            else:words+=[number_token(op['factor'])]
        words += [value(v)for v in op.get('targets',[])]
        words += ['_Cancel']if op.get('finish')=='Cancel'else['_Enter']if op['copy']else[]
        macro=' '.join(words);state['macro']=macro;return R.RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;original=join_probe.observe_command
    def observe(event_source,name,*args,**kw):return original(event_source,'Stretch'if name=='Taper'else name,*args,**kw)
    join_probe.observe_command=observe;local=dict(host);local['Rhino']=rhino
    try:
        value,elapsed=base.run(mapped(op),local);value['recipe']=op;value.update(state);return value,elapsed
    finally:join_probe.observe_command=original
