"""Owned OrientOnSrf GUI presets and per-placement prompts."""
import math
if globals().get('__package__'):
    from . import orient_surface_probe as base
else:
    import orient_surface_probe as base
FIELDS={'axes','uniform','scale_prompt','rotation_prompt'}
def stripped(op):
    if not isinstance(op.get('options'),dict):raise ValueError('surface options must be a dictionary')
    r=dict(op);r.pop('inherit',None);r.pop('setup',None);r['op']='orient_surface';r['options']={k:v for k,v in r['options'].items()if k not in FIELDS};return r
def validate(op):
    if not isinstance(op,dict)or op.get('op')!='orient_surface_options':raise ValueError('invalid surface options recipe')
    base.validate(stripped(op))
    if type(op.get('inherit',False))is not bool:raise ValueError('invalid inherited options')
    if op.get('inherit')and 'setup'not in op:raise ValueError('inherited options require an owned setup invocation')
    if 'setup'in op:
        child=dict(op);child['options']=child.pop('setup');child.pop('inherit',None);validate(child)
    for k in ('uniform','scale_prompt','rotation_prompt'):
        if type(op['options'].get(k,False))is not bool:raise ValueError('invalid dialog toggle')
    if 'axes'in op['options']:
        v=op['options']['axes']
        if not isinstance(v,list)or len(v)!=3 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in v):raise ValueError('invalid scale axes')
        if op['options'].get('uniform',True)and v!=[op['options'].get('scale',1)]*3:raise ValueError('uniform axes must agree with the scalar preset')
def run(op,host,iterations,tolerance):
    validate(op)
    from orient_surface_dialog import watch
    setup=None
    if 'setup'in op:
        seed=dict(op);seed['options']=seed.pop('setup');seed.pop('inherit',None)
        t,s=watch(seed['options'])
        try:
            def cancel(recipe,macro):
                return macro.split(' _Copy=_',1)[0].replace('_-OrientOnSrf','_OrientOnSrf',1)+' _Cancel'
            setup,_=base.run(stripped(seed),host,iterations,tolerance,cancel);setup['dialog']=s
        finally:t.Stop()
    timer,state=watch(op['options'],not op.get('inherit',False))
    def build(recipe,macro):
        prefix,tail=macro.split(' _Copy=_',1);points=[t for t in tail.split()if t.startswith('w')];o=op['options']
        result=prefix.replace('_-OrientOnSrf','_OrientOnSrf',1)+' _Copy=_'+('Yes'if o.get('copy',True)else'No')+' _Flip=_'+('Yes'if o.get('flip')else'No')+' _IgnoreTrims=_No '
        if not o.get('rigid',True):result+='_ConstrainNormal=_'+('Yes'if o.get('constrain_normal')else'No')+' '
        for point,entry in zip(points,[{}]+op.get('batch',[])):
            result+=point+' '
            if o.get('scale_prompt',False):result+=repr(float(entry.get('scale',o.get('scale',1))))+' '
            if o.get('rotation_prompt',False):result+=repr(float(entry.get('rotation',o.get('rotation',0))))+' '
        return result+('_Enter 'if o.get('copy',True)else'')+'_Cancel'
    try:
        value,elapsed=base.run(stripped(op),host,iterations,tolerance,build);value['dialog']=state;value['setup']=setup;return value,elapsed
    finally:timer.Stop()
