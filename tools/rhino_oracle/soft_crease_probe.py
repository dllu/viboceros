"""Fixed public SubDCrease commands, without proprietary implementation access."""
from copy import deepcopy
if __package__:
    from .soft_crease_cases import DISCOVERY, SCENES, all_cases
else:
    from soft_crease_cases import DISCOVERY, SCENES, all_cases


def request(discovery=True):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='soft_crease_command', id=key, case=key) for key in (DISCOVERY if discovery else SCENES)])


def validate(op):
    try:
        valid = isinstance(op, dict) and set(op) == {'op', 'id', 'case'} and op['op'] == 'soft_crease_command' and op['case'] in all_cases() and op['id'] == op['case']
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed soft-crease recipe required')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('one soft-crease workflow required')
    from subd_edit_scene import Scene
    from join_probe import observe_command
    case = deepcopy(all_cases()[op['case']])
    with Scene(case, host) as scene:
        scene.select()
        before = scene.snapshot()
        source_archive = scene.archive()
        tokens = ['_-SubDCrease']
        for action in case['actions']:
            if isinstance(action, dict):
                obj = scene.doc.Objects.FindId(scene.ids[action.get('source', 0)])
                v = scene.vertices(obj.Geometry)[action['vertex']]
                point = v.SurfacePoint() if action['space'] == 'surface' else v.ControlNetPoint
                tokens.append('{:.17g},{:.17g},{:.17g}'.format(point.X, point.Y, point.Z))
            elif isinstance(action, str):
                tokens.append('_' + action)
            else:
                tokens.append(str(action))
        tokens.extend(['_Cancel'] * 4)
        macro = ' '.join(tokens)
        marker = 'Owned soft crease ' + str(scene.S.Guid.NewGuid())
        scene.R.RhinoApp.WriteLine(marker)
        success, after, events = observe_command(scene.R.Commands.Command, 'SubDCrease',
            lambda: scene.R.RhinoApp.RunScript(macro, True), scene.snapshot, lambda: [], True)
        history = scene.R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        after_script, result_archive = scene.snapshot(), scene.archive()
        scene.R.RhinoApp.RunScript('_Undo', False)
        undone = scene.snapshot()
        scene.R.RhinoApp.RunScript('_Redo', False)
        redone = scene.snapshot()
        geometry = scene.doc.Objects.FindId(scene.ids[0]).Geometry
        public_methods = sorted(set(str(m.Name) for m in list(geometry.Edges)[0].GetType().GetMethods()))
        return dict(input=case, macro=macro, success=success, before=before, after=after,
                    after_script=after_script, undo=undone, redo=redone, events=events,
                    history=history, source_3dm=source_archive, result_3dm=result_archive,
                    public_edge_methods=public_methods), 0
