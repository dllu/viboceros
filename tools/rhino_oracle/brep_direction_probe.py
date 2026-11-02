"""Actual Dir edits on shared unedited 3DM sources in an owned idle document."""
import os
import re

SOURCES = ('holed', 'holed_reversed', 'warped', 'cylinder', 'sphere', 'cone', 'torus', 'box')
EDITS = ('u_reverse', 'v_reverse', 'swap_uv', 'flip')
SESSION_OPTIONS = ('UReverse','VReverse','SwapUV','Flip','FlipAll','NextMode','Mode=FlipU','Mode=FlipV','Mode=FlipNormal','Mode=SwapUV')


def validate(op):
    if (not isinstance(op, dict) or set(op)-{'macro', 'faces', 'sdk', 'sequence', 'cancel', 'pair'} != {'op', 'id', 'source', 'edit', 'preselect'}
            or op['op'] != 'brep_direction_command' or op['source'] not in SOURCES or op['edit'] not in EDITS
            or not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z', op['id']) is None
            or type(op['preselect']) is not bool or type(op.get('sdk',False)) is not bool
            or ('macro' in op and op['macro'] not in ('single', 'mode', 'mode_all', 'all'))):
        raise ValueError('invalid Brep Dir operation')
    if ('sequence' in op and (op.get('sdk',False) or op['source']=='box' or 'faces' in op or 'macro' in op
            or not isinstance(op['sequence'],list) or len(op['sequence']) > 8
            or any(step not in SESSION_OPTIONS for step in op['sequence']))):
        raise ValueError('invalid Dir session sequence')
    if 'cancel' in op and (type(op['cancel']) is not bool or 'sequence' not in op):
        raise ValueError('invalid Dir session ending')
    if 'pair' in op and (type(op['pair']) is not bool or 'sequence' not in op):
        raise ValueError('invalid paired Dir session')
    if op.get('sdk',False) and (op['edit']=='flip' or not op['preselect']):
        raise ValueError('SDK capture requires preselected face chart edits')
    if 'faces' in op and (not isinstance(op['faces'], list) or not op['faces']
            or any(type(i) is not int or not 0 <= i < (6 if op['source']=='box' else 1) for i in op['faces'])
            or len(set(op['faces'])) != len(op['faces'])):
        raise ValueError('invalid Dir face component selection')


def run(op, host, iterations, tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']; G = R.Geometry
    from join_probe import observe_command
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('Dir requires one iteration and an empty owned idle document')
    model = R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__), 'fixtures', 'brep_direction', op['source']+'.3dm'))
    if model is None or model.Objects.Count != 1: raise ValueError('shared Dir input unavailable')
    source = list(model.Objects)[0].Geometry.DuplicateBrep(); key = S.Guid.Empty
    keys = []
    group = -1
    def geometry(b):
        return host['_interchange_brep_record'](b)
    def object_snapshot(key):
        obj = doc.Objects.FindId(key)
        if obj is None: raise ValueError('Dir changed source identity')
        if not obj.Geometry.IsValid: raise ValueError('Dir produced invalid geometry')
        return dict(geometry=geometry(obj.Geometry), selected=bool(obj.IsSelected(False)),
                    name=obj.Attributes.Name, groups=int(obj.Attributes.GroupCount))
    def snapshot():
        if len(list(doc.Objects)) != len(keys): raise ValueError('Dir changed object count')
        rows = [object_snapshot(key) for key in keys]
        return rows if op.get('pair',False) else rows[0]
    try:
        attrs = R.DocObjects.ObjectAttributes();attrs.Name = 'Direction source'
        try:
            for _ in range(2 if op.get('pair',False) else 1):
                added = doc.Objects.AddBrep(source, attrs, None, False, False)
                if added == S.Guid.Empty: raise ValueError('Dir source insertion failed')
                keys.append(added)
            key = keys[0]
        finally: attrs.Dispose()
        if key == S.Guid.Empty: raise ValueError('Dir source insertion failed')
        group = doc.Groups.Add('Dir source group', keys)
        if group < 0: raise ValueError('Dir group insertion failed')
        if op['preselect']:
            if 'faces' in op and not op.get('sdk',False):
                obj = doc.Objects.FindId(key)
                for i in op['faces']:
                    if not obj.SelectSubObject(G.ComponentIndex(G.ComponentIndexType.BrepFace, i), True, True, False):
                        raise ValueError('Dir face preselection failed')
            else:
                for selected in keys: doc.Objects.Select(selected)
        doc.ClearUndoRecords(True)
        before = snapshot()
        edit = op['edit']; option = dict(u_reverse='UReverse', v_reverse='VReverse', swap_uv='SwapUV', flip='Flip')[edit]
        mode = dict(u_reverse='FlipU', v_reverse='FlipV', swap_uv='SwapUV', flip='FlipNormal')[edit]
        form = op.get('macro', 'single' if source.Faces.Count == 1 else 'mode_all')
        choices = dict(single=' _'+option, mode=' _Mode=_'+mode,
                       mode_all=' _Mode=_'+mode+' _FlipAll', all=' _FlipAll')[form]
        if 'sequence' in op: choices = ''.join(' _'+step.replace('=','=_') for step in op['sequence'])
        select = '' if op['preselect'] else ''.join(' _SelID '+str(selected) for selected in keys)+' _Enter'
        macro = '_-Dir'+select+choices+(' !' if op.get('cancel',False) else ' _Enter')
        marker = 'Owned Dir '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        if op.get('sdk',False):
            edited = source.DuplicateBrep()
            try:
                for index in op.get('faces',range(edited.Faces.Count)):
                    face = edited.Faces[index]
                    result = face.Transpose(True) if edit=='swap_uv' else face.Reverse(0 if edit=='u_reverse' else 1,True)
                    if result is None: raise ValueError('public face chart edit failed')
                if not edited.IsValid: raise ValueError('public face chart edit produced invalid B-rep')
                record = doc.BeginUndoRecord('Dir SDK chart replacement')
                try: success = bool(doc.Objects.Replace(key,edited))
                finally: doc.EndUndoRecord(record)
                after = snapshot(); events = []
            finally: edited.Dispose()
        else:
            success, after, events = observe_command(R.Commands.Command, 'Dir',
                lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1]
        if R.Commands.Command.InCommand(): R.RhinoApp.RunScript('!', False)
        R.RhinoApp.RunScript('_Undo', False); undo = snapshot()
        R.RhinoApp.RunScript('_Redo', False); redo = snapshot()
        return dict(success=bool(success), before=before, after=after, undo=undo, redo=redo,
                    events=events, history=history, macro=macro, sdk=op.get('sdk',False)), 0
    finally:
        if R.Commands.Command.InCommand(): R.RhinoApp.RunScript('!', False)
        for owned in keys: doc.Objects.Delete(owned, True)
        if group >= 0: doc.Groups.Delete(group)
        source.Dispose(); model.Dispose()
