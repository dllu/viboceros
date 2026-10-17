"""Bounded public Ellipse getter recipes and model/history observations."""


def validate(op):
    if globals().get('__package__'):
        from .getter_recipe import validate as validate_recipe
    else:
        from getter_recipe import validate as validate_recipe
    options={'Default','Vertical','Corner','Diameter','FromFoci','Deformable','Degree','PointCount','MarkFoci','AroundCurve','Enter','Cancel'}
    options.update(k+'='+v for k in ('Deformable','MarkFoci')for v in ('Yes','No'))
    validate_recipe(op,'ellipse_command',options,r'\A(?:Degree=[0-9]{1,2}|PointCount=(?:[0-9]{1,3}|1000))\Z')


def run(op, host, iterations, tolerance):
    validate(op)
    if globals().get('__package__'):
        from .curve_primitive_probe import run as run_primitive
    else:
        from curve_primitive_probe import run as run_primitive
    driver = setup_driver = None
    if op.get('live_getters'):
        from conic_getter_input import drive
        driver = lambda guides: drive(op, host, guides, command='Ellipse')
        if 'setup' in op:
            setup_driver = lambda guides: drive(dict(op, actions=op['setup'], id=op['id'] + '-setup'), host, guides, command='Ellipse')
    return run_primitive(op, host, iterations, tolerance, 'Ellipse', driver, setup_driver)
