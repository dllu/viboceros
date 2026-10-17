"""Bounded public Circle construction getters with complete history snapshots."""
def validate(op):
    if globals().get('__package__'):
        from .getter_recipe import validate as validate_recipe
    else:
        from getter_recipe import validate as validate_recipe
    operation=op.get('op')if isinstance(op,dict)else None
    if operation not in ('circle_around','circle_deform'):raise ValueError('invalid Circle getter recipe')
    options={'AroundCurve','Radius','Diameter','Circumference','Area','Orientation','3Point','2Point','Enter','Cancel'}
    pattern=None
    if operation=='circle_deform':
        options.update({'Deformable','Deformable=Yes','Deformable=No','PointCount','Degree','Vertical','FitPoints'})
        pattern=r'\A(?:Degree=[0-9]{1,2}|PointCount=(?:[0-9]{1,3}|1000))\Z'
    recipe=op
    if 'precise_length'in op:
        if operation!='circle_deform' or type(op['precise_length'])is not bool:raise ValueError('invalid Circle length accuracy flag')
        recipe=dict(op);recipe.pop('precise_length')
    validate_recipe(recipe,operation,options,pattern,numeric_options={'Radius','Diameter','Circumference','Area'},allow_points=operation=='circle_deform')
    if not op.get('live_getters'):raise ValueError('Circle getters require owned live input')


def run(op,host,iterations,tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    from conic_getter_input import drive
    driver=lambda guides:drive(op,host,guides,command='Circle')
    setup_driver=None
    if 'setup'in op:setup_driver=lambda guides:drive(dict(op,actions=op['setup'],id=op['id']+'-setup'),host,guides,command='Circle')
    return run_primitive(op,host,iterations,tolerance,'Circle',driver,setup_driver)
