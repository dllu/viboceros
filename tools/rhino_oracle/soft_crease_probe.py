"""Fixed public SubDCrease commands, without proprietary implementation access."""
from copy import deepcopy
if __package__:
    from .soft_crease_cases import DISCOVERY, SCENES, LABELS, EXTRA, PRECISION, NUMERIC_LABELS, all_cases
else:
    from soft_crease_cases import DISCOVERY, SCENES, LABELS, EXTRA, PRECISION, NUMERIC_LABELS, all_cases


def request(discovery=True, labels=False, extra=False, precision=False, numeric_labels=False):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='soft_crease_command', id=key, case=key) for key in (NUMERIC_LABELS if numeric_labels else PRECISION if precision else EXTRA if extra else LABELS if labels else DISCOVERY if discovery else SCENES)])


def validate(op):
    try:
        valid = isinstance(op, dict) and set(op) == {'op', 'id', 'case'} and op['op'] == 'soft_crease_command' and op['case'] in all_cases() and op['id'] == op['case']
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed soft-crease recipe required')


def run(op, host, iterations, tolerance):
    validate(op)
    if type(iterations) is not int or iterations != 1:
        raise ValueError('one soft-crease workflow required')
    from subd_edit_scene import Scene
    from join_probe import observe_command
    case = deepcopy(all_cases()[op['case']])
    with Scene(case, host) as scene:
        scene.select()
        before = scene.snapshot()
        source_archive = scene.archive()
        live_actions = []
        tokens = ['_-SubDCrease']
        for action in case['actions']:
            if isinstance(action, dict):
                obj = scene.doc.Objects.FindId(scene.ids[action.get('source', 0)])
                v = scene.vertices(obj.Geometry)[action['vertex']]
                point = v.SurfacePoint() if action['space'] == 'surface' else v.ControlNetPoint
                tokens.append('{:.17g},{:.17g},{:.17g}'.format(point.X, point.Y, point.Z))
                live_actions.append(dict(click=[float(point.X), float(point.Y), float(point.Z)]))
            elif isinstance(action, str):
                tokens.append('_' + action)
            else:
                from number_token import number_token
                tokens.append(number_token(action))
            if not isinstance(action, dict):
                live_actions.append(action)
        tokens.extend(['_Cancel'] * 4)
        macro = ' '.join(tokens)
        marker = 'Owned soft crease ' + str(scene.S.Guid.NewGuid())
        scene.R.RhinoApp.WriteLine(marker)
        trace = []
        if case.get('live'):
            from conic_getter_input import drive
            view = scene.doc.Views.ActiveView
            view.ActiveViewport.SetProjection(scene.R.Display.DefinedViewportProjection.Top, 'Owned soft crease input', False)
            view.ActiveViewport.ZoomExtents()
            if case.get('fit_sources'):
                points = []
                for key in scene.ids:
                    points.extend(v.ControlNetPoint for v in scene.vertices(scene.doc.Objects.FindId(key).Geometry))
                view.ActiveViewport.ZoomBoundingBox(scene.R.Geometry.BoundingBox(points))
            view.Redraw()
            def execute():
                result, details = drive(dict(op=op['op'], id=op['id'], actions=live_actions), host, [], command='SubDCrease')
                trace.append(details)
                return result
        else:
            execute = lambda: scene.R.RhinoApp.RunScript(macro, True)
        success, after, events = observe_command(scene.R.Commands.Command, 'SubDCrease',
            execute, scene.snapshot, lambda: [], True)
        history = scene.R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        after_script, result_archive = scene.snapshot(), scene.archive()
        roundtrip = []
        if case.get('roundtrip'):
            from subd_box_probe import subd_value
            model = scene.R.FileIO.File3dm.FromByteArray(scene.S.Convert.FromBase64String(result_archive))
            try:
                roundtrip = [dict(name=obj.Attributes.Name, subd=subd_value(obj.Geometry, host))
                             for obj in model.Objects if isinstance(obj.Geometry, scene.R.Geometry.SubD)]
            finally:
                model.Dispose()
        scene.R.RhinoApp.RunScript('_Undo', False)
        undone = scene.snapshot()
        scene.R.RhinoApp.RunScript('_Redo', False)
        redone = scene.snapshot()
        geometry = scene.doc.Objects.FindId(scene.ids[0]).Geometry
        public_methods = sorted(set(str(m.Name) for m in list(geometry.Edges)[0].GetType().GetMethods()))
        return dict(input=case, macro=macro, success=success, before=before, after=after,
                    after_script=after_script, undo=undone, redo=redone, events=events,
                    history=history, source_3dm=source_archive, result_3dm=result_archive,
                    public_edge_methods=public_methods, input_trace=trace, roundtrip=roundtrip), 0
