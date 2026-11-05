"""Fixed Length/SelChain recipes using independently specified owned geometry."""
def cases():
    rows = [dict(op='length_chain',id='length_chain_'+scene+'_'+str(seed),scene=scene,
        seed=seed,pre=[],steps=[dict(kind='click',modifiers='plain'),dict(kind='key',value='Enter')],
        finish='Enter',options=dict(continuity=continuity))
        for scene,seed,continuity in [('lines',1,'Tangency'),('corner',0,'Tangency'),
            ('surface',0,'Position'),('mixed',1,'Tangency'),('mixed',0,'Position'),
            ('branch',0,'Position')]]
    rows[-1]['steps'].append(dict(kind='key',value='Enter'))
    return rows

def validate(op):
    if not isinstance(op,dict) or op not in cases()+controls()+followups() or type(op.get('seed')) is not int or op.get('pre')!=[] or any('source' in step and type(step['source']) is not int for step in op.get('steps',[])):
        raise ValueError('fixed owned Length chain required')


def controls():
    click=dict(kind='click',modifiers='plain')
    key=lambda value:dict(kind='key',value=value)
    rows=[]
    for name,scene,seed,steps,finish,continuity in [
        ('inner_cancel_empty','lines',1,[key('Escape')],'Cancel','Tangency'),
        ('inner_cancel_after_seed','lines',1,[click,key('Escape')],'Cancel','Tangency'),
        ('outer_cancel_after_chain','lines',1,[click,key('Enter')],'Cancel','Tangency'),
        ('undo_nested','lines',1,[click,key('Undo'),key('Enter')],'Enter','Tangency'),
        ('branch_alternative','branch',0,[click,dict(click,source=1),key('Enter')],'Enter','Position'),
        ('outer_reenter','lines',1,[click,key('Enter'),key('SelChain'),dict(click,source=3),key('Enter')],'Enter','Tangency'),
        ('units_after_chain','lines',1,[click,key('Enter'),key('Units=Centimeters')],'Enter','Tangency')]:
        rows.append(dict(op='length_chain',id=name,scene=scene,seed=seed,pre=[],steps=steps,finish=finish,options=dict(continuity=continuity)))
    return rows


def followups():
    rows=[]
    for name,scene,seed,steps,finish in [
        ('settled_cancel_empty','lines',1,[dict(kind='key',value='Escape')],'Cancel'),
        ('settled_cancel_after_seed','lines',1,[dict(kind='click',modifiers='plain'),dict(kind='key',value='Escape')],'Cancel'),
        ('units_menu','lines',1,[dict(kind='click',modifiers='plain')]+[dict(kind='key',value=k)for k in ('Enter','Units','Centimeter')],'Enter'),
        ('mixed_units_menu','mixed',1,[dict(kind='click',modifiers='plain')]+[dict(kind='key',value=k)for k in ('Enter','Units','Centimeter')],'Enter')]:
        rows.append(dict(op='length_chain',id=name,scene=scene,seed=seed,pre=[],steps=steps,finish=finish,options=dict(continuity='Tangency')))
    return rows
