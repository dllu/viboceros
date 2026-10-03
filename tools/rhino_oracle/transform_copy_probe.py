"""Owned repeated transform commands, terminal events, and external Undo/Redo."""
import math
import re

COMMANDS = ('Scale', 'Scale1D', 'Scale2D', 'Rotate', 'Rotate3D', 'Mirror', 'Shear')
MIRROR_OPTIONS = ('3Point', 'XAxis', 'YAxis', 'ZAxis', 'Object')


def finite(value):
    try:
        return type(value) in (int, float) and not math.isnan(value) and not math.isinf(value)
    except (OverflowError, TypeError):
        return False


def validate(operation):
    if (not isinstance(operation, dict)
            or not {'op', 'id', 'command', 'sources', 'grouped', 'inputs', 'finish', 'undo_redo', 'sel_last'} <= set(operation)
            or not set(operation) <= {'op', 'id', 'command', 'sources', 'grouped', 'inputs', 'finish', 'undo_redo', 'sel_last', 'selected', 'cplane', 'mirror_target', 'source_selection'}
            or operation['op'] != 'transform_copy_command'
            or not isinstance(operation['id'], str) or re.match(r'^[A-Za-z0-9_.-]{1,100}\Z', operation['id']) is None
            or operation['command'] not in COMMANDS
            or not isinstance(operation['sources'], list) or not 1 <= len(operation['sources']) <= 16
            or any(type(operation[key]) is not bool for key in ('grouped', 'undo_redo', 'sel_last'))
            or operation['finish'] not in ('Enter', 'Cancel', 'Automatic')
            or not isinstance(operation['inputs'], list)
            or not (0 if 'source_selection' in operation else 1) <= len(operation['inputs']) <= 32):
        raise ValueError('invalid transform Copy workflow')
    if 'source_selection' in operation:
        steps = operation['source_selection']
        if not isinstance(steps, list) or not 1 <= len(steps) <= 32:
            raise ValueError('invalid transform source selection steps')
        for step in steps:
            if type(step) is int and 0 <= step < len(operation['sources']):
                continue
            if isinstance(step, str) and step in ('SelAll', 'SelNone', 'Enter', 'Cancel', 'Copy=Yes', 'Copy=No', '3Point', 'XAxis', 'YAxis', 'ZAxis', 'Object'):
                continue
            raise ValueError('source selection requires bounded owned indices or named inputs')
        if operation.get('selected'):
            raise ValueError('command-first transform requires empty preselection')
    if 'mirror_target' in operation:
        if __package__:
            from .mirror_object_probe import validate_target
        else:
            from mirror_object_probe import validate_target
        if operation['command'] != 'Mirror':
            raise ValueError('plane targets require Mirror')
        validate_target(operation['mirror_target'])
        if operation['inputs'].count('Target') > 1:
            raise ValueError('Mirror target capture delivers one prescribed pick')
    for source in operation['sources']:
        if (not isinstance(source, list) or len(source) != 3
                or any(not finite(value) for value in source)):
            raise ValueError('transform sources require finite points')
    selected = operation.get('selected', [] if 'source_selection' in operation else list(range(len(operation['sources']))))
    if (not isinstance(selected, list) or (not selected and 'source_selection' not in operation)
            or any(type(index) is not int or not 0 <= index < len(operation['sources']) for index in selected)
            or len(set(selected)) != len(selected)):
        raise ValueError('invalid transform preselection')
    if 'cplane' in operation:
        plane = operation['cplane']
        if (not isinstance(plane, dict) or set(plane) != {'origin', 'x_axis', 'y_axis'}
                or any(not isinstance(values, list) or len(values) != 3 or any(not finite(value) for value in values) for values in plane.values())
                or any(abs(sum(value*value for value in plane[key])-1) > 1e-9 for key in ('x_axis', 'y_axis'))
                or abs(sum(x*y for x, y in zip(plane['x_axis'], plane['y_axis']))) > 1e-9):
            raise ValueError('invalid transform construction plane')
    for token in operation['inputs']:
        if not isinstance(token, str) or not 1 <= len(token) <= 100:
            raise ValueError('invalid transform input')
        if token in ('Copy=Yes', 'Copy=No', 'Undo', 'Enter'):
            continue
        if operation['command'] == 'Mirror' and token in MIRROR_OPTIONS:
            continue
        if token == 'Target' and 'mirror_target' in operation:
            continue
        coordinates = token[1:] if token.startswith('w') else token
        if re.match(r'^[-+0-9.eE]+(?:,[-+0-9.eE]+){0,2}\Z', coordinates) is None:
            raise ValueError('transform inputs are bounded numeric values or Copy options')
        try:
            values = [float(value) for value in coordinates.split(',')]
        except ValueError:
            raise ValueError('invalid transform number')
        if any(math.isnan(value) or math.isinf(value) for value in values):
            raise ValueError('nonfinite transform number')
        if token.startswith('w') and len(values) != 3:
            raise ValueError('world transform points require three coordinates')


def run(operation, host):
    if 'mirror_target' in operation:
        from mirror_object_probe import run_with_target
        return run_with_target(operation, host, run_owned)
    return run_owned(operation, host)


def run_owned(operation, host, target=None):
    from join_probe import observe_command
    validate(operation)
    Rhino, System = host['Rhino'], host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.HiddenObjects = settings.LockedObjects = True
    objects = lambda: list(doc.Objects.GetObjectList(settings))
    baseline = set(obj.Id for obj in objects())
    selected = [obj.Id for obj in objects() if obj.IsSelected(False)]
    baseline_groups = set(doc.Groups[index].Id for index in range(doc.Groups.Count) if not doc.Groups[index].IsDeleted)
    ids = []
    def groups():
        return [index for index in range(doc.Groups.Count)
                if not doc.Groups[index].IsDeleted and doc.Groups[index].Id not in baseline_groups]
    def snapshot():
        owned = sorted((obj for obj in objects() if obj.Id not in baseline), key=lambda obj: obj.RuntimeSerialNumber)
        owned_groups = groups()
        rows = []
        for obj in owned:
            if not isinstance(obj.Geometry, Rhino.Geometry.Point):
                raise ValueError('unexpected transform geometry')
            attributes = obj.Attributes
            rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)), point=host['_xyz'](obj.Geometry.Location),
                name=attributes.Name, color=[int(attributes.ObjectColor.R), int(attributes.ObjectColor.G), int(attributes.ObjectColor.B)],
                color_source=str(attributes.ColorSource), current_layer=attributes.LayerIndex == doc.Layers.CurrentLayerIndex,
                groups=[owned_groups.index(group) for group in (attributes.GetGroupList() or [])]))
        return dict(objects=rows, groups=[dict(members=[index for index, obj in enumerate(owned)
            if group in (obj.Attributes.GetGroupList() or [])]) for group in owned_groups])
    try:
        plane = Rhino.Geometry.Plane.WorldXY
        if 'cplane' in operation:
            values = operation['cplane']
            plane = Rhino.Geometry.Plane(host['_point'](values['origin']), host['_vector'](values['x_axis']), host['_vector'](values['y_axis']))
            if not plane.IsValid:
                raise ValueError('invalid transform construction plane')
        doc.Views.ActiveView.ActiveViewport.SetConstructionPlane(plane)
        doc.Objects.UnselectAll()
        serial = doc.BeginUndoRecord('Viboceros transform sources')
        try:
            for index, source in enumerate(operation['sources']):
                attributes = Rhino.DocObjects.ObjectAttributes()
                try:
                    attributes.Name = 'source-%d' % index
                    attributes.ObjectColor = System.Drawing.Color.FromArgb(10+index, 30, 50)
                    attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                    key = doc.Objects.AddPoint(Rhino.Geometry.Point3d(*source), attributes)
                finally:
                    attributes.Dispose()
                if key == System.Guid.Empty:
                    raise ValueError('transform source insertion failed')
                ids.append(key)
            if operation['grouped'] and doc.Groups.Add('RepeatSource_'+operation['id'], ids) < 0:
                raise ValueError('transform source grouping failed')
            for index in operation.get('selected', [] if 'source_selection' in operation else list(range(len(ids)))):
                doc.Objects.Select(ids[index])
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        tokens = []
        for step in operation.get('source_selection', []):
            if type(step) is int:
                tokens.append('_SelID '+str(ids[step]))
            else:
                tokens.append('_'+step.replace('=Yes', '=_Yes').replace('=No', '=_No'))
        for token in operation['inputs']:
            if token == 'Target':
                tokens.append('_SelID '+str(target['id']) if target['pick'] == 'id' else '_Pause')
            else:
                tokens.append('_'+token.replace('=Yes', '=_Yes').replace('=No', '=_No') if token.startswith('Copy=') or token in ('Undo', 'Enter') or token in MIRROR_OPTIONS else token)
        macro = '_'+operation['command']+' '+' '.join(tokens)
        if operation['finish'] != 'Automatic':
            macro += ' _'+operation['finish']
        marker = 'Viboceros transform '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress']('transform '+operation['id']+' '+macro)
        action = lambda: Rhino.RhinoApp.RunScript(macro, True)
        if target is not None and target['pick'] in ('mouse', 'mouse-sub') and 'Target' in operation['inputs']:
            from mirror_object_probe import click_target
            action = lambda: click_target(operation, target, host, macro)
        succeeded, after, events = observe_command(Rhino.Commands.Command, operation['command'],
            action, snapshot,
            lambda: [ids.index(obj.Id) if obj.Id in ids else None for obj in objects() if obj.IsSelected(False)]
                if 'source_selection' in operation else [], True)
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        last = undo = redo = None
        if operation['sel_last']:
            Rhino.RhinoApp.RunScript('_SelLast', True)
            last = snapshot()
        geometry_state = lambda state: dict(groups=state['groups'], objects=[dict((key,value) for key,value in obj.items() if key != 'selected') for obj in state['objects']])
        if operation['undo_redo'] and geometry_state(after) != geometry_state(before):
            Rhino.RhinoApp.RunScript('_Undo', True)
            undo = snapshot()
            Rhino.RhinoApp.RunScript('_Redo', True)
            redo = snapshot()
        return dict(before=before, after=after, last=last, undo=undo, redo=redo,
            succeeded=succeeded, history=history, events=events,
            group_names=[doc.Groups[index].Name for index in groups()]), 0
    finally:
        errors = []
        for obj in objects():
            if obj.Id not in baseline:
                try:
                    if not doc.Objects.Delete(obj.Id, True):
                        errors.append('transform object cleanup failed')
                except Exception as error:
                    errors.append(str(error))
        for index in groups():
            try:
                if not doc.Groups.Delete(index):
                    errors.append('transform group cleanup failed')
            except Exception as error:
                errors.append(str(error))
        doc.Objects.UnselectAll()
        for key in selected:
            if doc.Objects.FindId(key) is not None:
                doc.Objects.Select(key)
        if errors:
            raise ValueError('; '.join(errors))
