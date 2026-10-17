"""Bounded public Circle AroundCurve getters with complete history snapshots."""
def validate(op):
    if globals().get('__package__'):
        from .getter_recipe import validate as validate_recipe
    else:
        from getter_recipe import validate as validate_recipe
    validate_recipe(op,'circle_around',{'AroundCurve','Radius','Diameter','Circumference','Area','Orientation','3Point','2Point','Enter','Cancel'},numeric_options={'Radius','Diameter','Circumference','Area'})
    if not op.get('live_getters'):raise ValueError('Circle AroundCurve requires owned live input')


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
