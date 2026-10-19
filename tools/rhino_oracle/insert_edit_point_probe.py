"""Public InsertEditPoint commands over the shared immutable curve sources."""
try:
    from . import insert_kink_probe as base
except (ImportError, ValueError):
    import insert_kink_probe as base


def plain(op):
    return dict(op, op='insert_kink_command')


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'insert_edit_point_command':
        raise ValueError('invalid edit-point insertion recipe')
    base.validate(plain(op))
    if op['family'] not in ('cubic','rational','circle','ellipse','line','polyline','polycurve','periodic'):
        raise ValueError('edit-point insertion requires a curve')
    if any(k in op for k in ('direction','symmetrical','midpoints','face','v_fraction')):
        raise ValueError('edit-point insertion has no surface/knot options')


class Forward:
    def __init__(self, target): self.target = target
    def __getattr__(self, name): return getattr(self.target, name)


def run(op, host, iterations, tolerance):
    validate(op)
    import join_probe
    R=host['Rhino']; app=Forward(R.RhinoApp); rhino=Forward(R); rhino.RhinoApp=app
    state={}
    def execute(macro, echo):
        if not macro.startswith('_InsertKink '): return R.RhinoApp.RunScript(macro, echo)
        macro=macro.replace('_InsertKink ', '_InsertEditPoint ', 1)
        state['macro']=macro
        return R.RhinoApp.RunScript(macro, echo)
    app.RunScript=execute
    original=join_probe.observe_command
    def observe(event_source, command, *args, **kwargs):
        return original(event_source,'InsertEditPoint' if command=='InsertKink' else command,*args,**kwargs)
    join_probe.observe_command=observe
    local=dict(host);local['Rhino']=rhino
    try:
        value,elapsed=base.run(plain(op),local,iterations,tolerance)
        value.update(recipe=op,macro=state['macro'],tokens=['w'+','.join('%.17g'%x for x in p)for p in value['picks']]+['!'if op.get('cancel')else'_Enter'])
        return value,elapsed
    finally: join_probe.observe_command=original
