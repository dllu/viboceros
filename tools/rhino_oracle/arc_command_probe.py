"""Bounded public Arc construction getters and actual model history."""


def validate(op):
    if globals().get('__package__'):
        from .getter_recipe import validate as validate_recipe
    else:
        from getter_recipe import validate as validate_recipe
    options={'Center','StartPoint','ThroughPoint','Direction','Midpoint','Tilted','Length','Enter','Cancel','Deformable','Degree','PointCount'}
    options.update('Direction='+v for v in ('Clockwise','Counterclockwise'))
    actions=op.get('actions',[])if isinstance(op,dict)else[]
    normalized=dict(op,actions=[a.get('click',a.get('hover'))if isinstance(a,dict)and(set(a)=={'click'}or set(a)=={'hover'})else a for a in actions])if isinstance(op,dict)else op
    if isinstance(op,dict):
        if type(op.get('record_preferences',False))is not bool:raise ValueError('invalid Arc preference witness flag')
        normalized.pop('record_preferences',None)
    validate_recipe(normalized,'arc_command',options,numeric_pattern=r'\A(?:Degree=[0-9]{1,2}|PointCount=(?:[0-9]{1,3}|1000))\Z',numeric_options=('Length',))
    if any(isinstance(a,dict)and('click'in a or'hover'in a)for a in op.get('setup',[])):
        raise ValueError('Arc setup clicks are not supported')
    for index,a in enumerate(actions):
        if isinstance(a,dict)and('click'in a or'hover'in a):
            if 'Tilted'not in actions or not isinstance(actions[-1],dict)or'click'not in actions[-1]or('click'in a and index!=len(actions)-1):
                raise ValueError('Arc pointer input requires the final Tilted endpoint getter')
            tilt=actions.index('Tilted')
            if len([p for p in actions[:tilt]if isinstance(p,list)])!=1 or index<tilt+3 or not all(isinstance(p,list)for p in actions[tilt+1:tilt+3])or not all(isinstance(p,dict)and set(p)=={'hover'}for p in actions[tilt+3:-1]):
                raise ValueError('Arc endpoint clicks require explicit center, start and orientation points')
    if not op.get('live_getters') or op.get('curves'):
        raise ValueError('Arc construction requires owned typed getters without source picks')


def run(op,host,iterations,tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    from conic_getter_input import drive
    driver=lambda guides:drive(op,host,guides,command='Arc')
    setup_driver=None
    if 'setup'in op:
        setup_driver=lambda guides:drive(dict(op,actions=op['setup'],id=op['id']+'-setup'),host,guides,command='Arc')
    result,elapsed=run_primitive(op,host,iterations,tolerance,'Arc',driver,setup_driver)
    if op.get('record_preferences'):
        returned,trace=drive(dict(op,actions=['Deformable','Cancel'],id=op['id']+'-preferences'),host,[],command='Arc')
        if trace['errors']or trace['completed']!=2:raise ValueError('Arc preference witness did not complete')
        result['preference_input_trace']=trace
    return result,elapsed
