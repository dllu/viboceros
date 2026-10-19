"""Actual RemoveControlPoint getters; sources and snapshots reuse owned probes."""
import math
try:
    from . import remove_knot_command_probe as base
except (ImportError, ValueError):
    import remove_knot_command_probe as base


def plain(op):
    return dict(op, op='remove_knot_command', actions=[])


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'remove_control_point_command':
        raise ValueError('invalid control removal recipe')
    base.validate(plain(op))
    actions = op.get('actions')
    if not isinstance(actions, list) or sum(isinstance(a, dict) for a in actions) > 1 or len(actions) > 4:
        raise ValueError('invalid control removal actions')
    for a in actions:
        if isinstance(a, str):
            if a not in ('Toggle', 'Direction=U', 'Direction=V'):
                raise ValueError('invalid control removal option')
        elif isinstance(a, dict) and set(a) == {'grip'}:
            values = a['grip'] if isinstance(a['grip'], list) else [a['grip']]
            if len(values) not in (1, 2) or any(type(i) is not int or not 0 <= i <= 256 for i in values):
                raise ValueError('invalid control grip')
        elif isinstance(a, dict) and set(a) == {'point'}:
            p = a['point']
            if not isinstance(p, list) or len(p) != 3 or any(type(x) not in (int, float) or math.isnan(x) or math.isinf(x) or abs(x) > 1e6 for x in p):
                raise ValueError('invalid control removal point')
        else:
            raise ValueError('invalid control removal action')


def run(op, host, iterations, tolerance):
    validate(op)
    import join_probe
    R = host['Rhino']; G = R.Geometry; doc = R.RhinoDoc.ActiveDoc
    app = base.Forward(R.RhinoApp); rhino = base.Forward(R); rhino.RhinoApp = app
    state = {}; owned = []

    def execute(macro, echo):
        if not macro.startswith('_RemoveKnot '):
            return R.RhinoApp.RunScript(macro, echo)
        source = list(doc.Objects)[0]
        geometry = source.Geometry
        surface = op['family'] in ('surface', 'surface_rational', 'trimmed_rectangle', 'trimmed_hole', 'box')
        target = geometry.Faces[op.get('face', 0)].UnderlyingSurface() if isinstance(geometry, G.Brep) else geometry
        target = target.ToNurbsSurface() if surface else target.ToNurbsCurve()
        owned.append(target)
        prefix = '_RemoveControlPoint '
        if not op.get('preselect', True):
            prefix += '_SelID ' + str(source.Id) + ' '
        # A linear tensor direction forces removal on the other axis in Rhino;
        # the getter does not offer Direction or Toggle in that case.
        has_direction = surface and target.Points.CountU > 2 and target.Points.CountV > 2
        tokens = ['_Direction=_' + op.get('direction', 'U')] if has_direction else []
        picks = []
        for a in op['actions']:
            if isinstance(a, str):
                tokens.append('_' + a.replace('=', '=_'))
                continue
            if 'point' in a:
                p = a['point']
            else:
                i = a['grip']
                cv = target.Points.GetControlPoint(*i) if surface else target.Points[i]
                p = host['_xyz'](cv.Location)
            picks.append(p)
            tokens.append('w' + ','.join('%.17g' % x for x in p))
        if op.get('cancel'):
            tokens.append('!')
        elif not picks or op['family'] == 'box':
            tokens.append('_Enter')
        state.update(macro=prefix + ' '.join(tokens), tokens=tokens, picks=picks)
        return R.RhinoApp.RunScript(state['macro'], echo)

    app.RunScript = execute
    original = join_probe.observe_command
    def observe(event_source, command, *args, **kwargs):
        return original(event_source, 'RemoveControlPoint' if command == 'RemoveKnot' else command, *args, **kwargs)
    join_probe.observe_command = observe
    local = dict(host); local['Rhino'] = rhino
    try:
        value, elapsed = base.run(plain(op), local, iterations, tolerance)
        value.update(state); value['recipe'] = op
        return value, elapsed
    finally:
        join_probe.observe_command = original
        for g in reversed(owned):
            if g is not None:
                g.Dispose()
