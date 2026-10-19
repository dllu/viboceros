"""Owned InsertControlPoint commands with exact source and history snapshots."""
import math
try:
    from . import remove_knot_command_probe as base
except (ImportError, ValueError):
    import remove_knot_command_probe as base


def plain(op):
    return dict(op, op='remove_knot_command', actions=[], direction='U')


def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'insert_control_point_command':
        raise ValueError('invalid control insertion recipe')
    base.validate(plain(op))
    if op.get('direction', 'U') not in ('U', 'V', 'Both'):
        raise ValueError('invalid insertion direction')
    if 'subcurve' in op:
        raise ValueError('control insertion has no SubCrv option')
    actions = op.get('actions')
    if not isinstance(actions, list) or len(actions) > 16:
        raise ValueError('invalid insertion actions')
    for action in actions:
        if isinstance(action, str):
            if action not in ('Toggle', 'Direction=U', 'Direction=V', 'Direction=Both',
                              'Midpoint=Yes', 'Midpoint=No', 'Extend', 'Undo'):
                raise ValueError('invalid insertion option')
        elif isinstance(action, dict) and set(action) in ({'point'}, {'world'}):
            key = next(iter(action))
            p = action[key]
            values = p if isinstance(p, list) else [p]
            if len(values) not in ((3,) if key == 'world' else (1, 2)) or any(
                    type(x) not in (int, float) or math.isnan(x) or math.isinf(x)
                    or (abs(x) > 1e6 if key == 'world' else not 0 <= x <= 1)
                    for x in values):
                raise ValueError('invalid insertion point')
        else:
            raise ValueError('invalid insertion action')


def run(op, host, iterations, tolerance):
    validate(op)
    import join_probe
    R = host['Rhino']; G = R.Geometry; doc = R.RhinoDoc.ActiveDoc
    app = base.Forward(R.RhinoApp); rhino = base.Forward(R); rhino.RhinoApp = app
    state = {}

    def execute(macro, echo):
        if not macro.startswith('_RemoveKnot '):
            return R.RhinoApp.RunScript(macro, echo)
        obj = list(doc.Objects)[0]; source = obj.Geometry
        surface = op['family'] in ('surface', 'surface_rational', 'trimmed_rectangle', 'trimmed_hole', 'box')
        target = source.Faces[op.get('face', 0)] if isinstance(source, G.Brep) else source
        prefix = '_InsertControlPoint '
        if not op.get('preselect', True):
            prefix += '_SelID ' + str(obj.Id) + ' '
        tokens = ['_Direction=_' + op.get('direction', 'U')] if surface else []
        tokens.append('_Midpoint=_No')
        picks = []
        for action in op['actions']:
            if isinstance(action, str):
                tokens.append('_' + action.replace('=', '=_'))
                continue
            if 'world' in action:
                p = action['world']
            else:
                f = action['point']; uv = f if isinstance(f, list) else [f, op.get('v_fraction', .6)]
                p = host['_xyz'](target.PointAt(target.Domain(0).ParameterAt(uv[0]), target.Domain(1).ParameterAt(uv[1]))
                                 if surface else source.PointAt(source.Domain.ParameterAt(uv[0])))
            picks.append(p)
            tokens.append('w' + ','.join('%.17g' % x for x in p))
        tokens.append('!' if op.get('cancel') else '_Enter')
        state.update(macro=prefix + ' '.join(tokens), tokens=tokens, picks=picks)
        return R.RhinoApp.RunScript(state['macro'], echo)

    app.RunScript = execute
    original = join_probe.observe_command
    def observe(event_source, command, *args, **kwargs):
        return original(event_source, 'InsertControlPoint' if command == 'RemoveKnot' else command, *args, **kwargs)
    join_probe.observe_command = observe
    local = dict(host); local['Rhino'] = rhino
    try:
        value, elapsed = base.run(plain(op), local, iterations, tolerance)
        value.update(state); value['recipe'] = op
        return value, elapsed
    finally:
        join_probe.observe_command = original
