"""Bounded public Arc Extension selection, construction and history."""
def validate(op):
    if globals().get('__package__'):
        from .getter_recipe import validate as validate_recipe
    else:
        from getter_recipe import validate as validate_recipe
    options={'Extension','Center','Length','Enter','Cancel','Deformable','Degree','PointCount','Direction=Clockwise','Direction=Counterclockwise'}
    actions=op.get('actions',[])if isinstance(op,dict)else[]
    normalized=dict(op,actions=[a.get('click')if isinstance(a,dict)and set(a)=={'click'}else a for a in actions])if isinstance(op,dict)else op
    validate_recipe(normalized,'arc_extension',options,numeric_pattern=r'\A(?:Degree=[0-9]{1,2}|PointCount=(?:[0-9]{1,3}|1000))\Z',numeric_options=('Length',),allow_points=True,allow_curve_locations=True)
    for index,a in enumerate(actions):
        if isinstance(a,dict)and'click'in a:
            if set(a)!={'click'}or index!=len(actions)-1 or'Center'not in actions:raise ValueError('Arc Extension endpoint click requires a completed Center prefix')
    if not op.get('live_getters'):raise ValueError('Arc Extension requires owned live input')
    if any(isinstance(a,dict)and'curve'in a and'pick'not in a for a in op['actions']):raise ValueError('Arc Extension requires located source picks')

def run(op,host,iterations,tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    from conic_getter_input import drive
    return run_primitive(op,host,iterations,tolerance,'Arc',lambda guides:drive(op,host,guides,command='Arc'))
