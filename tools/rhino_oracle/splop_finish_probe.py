"""Separate native Enter completion from a later, idle Cancel command."""
try:
    from . import splop_probe as whole,splop_grips_probe as grips
    from .splop_reference_probe import Forward
except (ImportError,ValueError):
    import splop_probe as whole,splop_grips_probe as grips
    from splop_reference_probe import Forward

def mapped(op):
    if op['kind']=='Whole':return dict(op='splop_command',id=op['id'],shape=op['source'],surface=op['surface'],copy=True,rigid=False,flip=False,grouped=False,angle=0.,radius=4.,postselect=op['postselect'],finish='Cancel'if op['finish']=='Cancel'else'Complete')
    return dict(op='splop_grips',id=op['id'],source=op['source'],surface=op['surface'],copy=True,rigid=False,flip=False,selected=[0,1],edit=op['edit'],parent=op['parent'],finish='Cancel'if op['finish']=='Cancel'else'Complete')

def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','kind','source','surface','postselect','edit','parent','finish'}or op['op']!='splop_finish'or op['kind']not in('Whole','Grips')or op['finish']not in('Enter','Cancel'):raise ValueError('invalid Splop finish recipe')
    if any(type(op[k])is not bool for k in('postselect','edit','parent')):raise ValueError('invalid finish flags')
    if op['kind']=='Whole'and(op['edit']or op['parent']):raise ValueError('whole finish flags')
    if op['kind']=='Grips'and op['postselect']:raise ValueError('grip finish postselect')
    (whole if op['kind']=='Whole'else grips).validate(mapped(op))

def run(op,host,iterations,tolerance):
    validate(op);recipe=mapped(op);rhino=Forward(host['Rhino']);app=Forward(rhino.RhinoApp);rhino.RhinoApp=app;state={}
    def execute(macro,echo):
        if macro.startswith('_Splop '):
            if op['finish']=='Enter':
                if not macro.endswith(' _Enter _Cancel'):raise ValueError('missing recorded tail')
                macro=macro[:-len(' _Cancel')]
            state['executed_macro']=macro
        return host['Rhino'].RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;local=dict(host);local['Rhino']=rhino
    value,elapsed=(whole if op['kind']=='Whole'else grips).run(recipe,local,iterations,tolerance)
    value.update(state);value['recipe']=recipe;return value,elapsed
