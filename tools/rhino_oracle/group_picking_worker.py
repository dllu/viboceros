# -*- coding: utf-8 -*-
"""Dedicated idle-event worker; host validates its bounded three-line cases."""
import Rhino
import System
import os
import json
import time

root = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(root, 'request.json')) as stream: request = json.load(stream)
document = Rhino.RhinoDoc.ActiveDoc
state = dict(index=0, stage='setup', ids=[], groups=[], layers=[], results=[], busy=False, finished=False)

def progress(message, required=False):
    try:
        with open(os.path.join(root, 'worker-progress.log'), 'a') as stream: stream.write(message + '\n')
    except Exception:
        if required: raise  # PICK is an input request, not an optional diagnostic.
        pass  # Diagnostics must not prevent response publication or cleanup.

def cleanup():
    # Detach ownership first: finalization may be entered after a failed cleanup.
    ids, groups, layers = state['ids'], state['groups'], state['layers']
    state['ids'], state['groups'], state['layers'] = [], [], []
    errors = []
    def attempt(label, action):
        try:
            action()
        except Exception as error:
            errors.append('%s: %s' % (label, error))
    def restore_layer(index):
        layer = document.Layers[index]
        layer.IsLocked = False
        layer.IsVisible = True
        layer.CommitChanges()
    def delete(label, action):
        def checked():
            if not action(): raise RuntimeError('deletion returned false')
        attempt(label, checked)
    for index in layers: attempt('restore layer', lambda: restore_layer(index))
    for key in ids:
        attempt('unlock object', lambda: document.Objects.Unlock(key, True))
        attempt('show object', lambda: document.Objects.Show(key, True))
        delete('delete object', lambda: document.Objects.Delete(key, True))
    for group in groups: delete('delete group', lambda: document.Groups.Delete(group))
    for layer in layers: delete('delete layer', lambda: document.Layers.Delete(layer, True))
    return errors

def finish(error=None):
    if state['finished']: return
    state['finished'] = True
    errors = [error] if error else []
    try:
        Rhino.RhinoApp.Idle -= on_idle
    except Exception as failure:
        errors.append('detach idle callback: %s' % failure)
    errors.extend(cleanup())
    response = dict(protocol_version=1, engine='rhino', engine_version=str(Rhino.RhinoApp.Version), iterations=1, results=state['results'])
    if errors:
        response['error'] = '; '.join(errors)
        response['results'] = []
    temporary = os.path.join(root, 'response.json.tmp')
    with open(temporary, 'w') as stream: json.dump(response, stream)
    os.rename(temporary, os.path.join(root, 'response.json'))
    progress('worker: response published')
    try:
        document.Modified = False
        Rhino.RhinoApp.RunScript('_Exit _No', False)
    except Exception as failure:
        progress('worker exit failed: %s' % failure)

def last_snapshot(ids):
    records = []
    for key in ids:
        obj = document.Objects.FindId(key)
        if obj is None:
            records.append(None)
            continue
        point = obj.Geometry.PointAtStart
        layer = document.Layers[obj.Attributes.LayerIndex]
        records.append(dict(point=[float(point.X), float(point.Y), float(point.Z)],
            selected=bool(obj.IsSelected(False)), mode=str(obj.Attributes.Mode),
            layer_visible=layer.IsVisible, layer_locked=layer.IsLocked))
    return records

def last_sequence(operation, ids):
    records = [last_snapshot(ids)]
    for number, step in enumerate(operation['last_steps']):
        kind = step['kind']
        if kind == 'select':
            document.Objects.UnselectAll()
            for index in step['objects']:
                if not document.Objects.Select(ids[index]): raise ValueError('last-selection source is not selectable')
        elif kind == 'recall':
            script = '_SelLast' if step.get('deselect_others') is None else '_-SelLast _DeselectOthersBeforeSelect=_%s _Enter' % ('Yes' if step['deselect_others'] else 'No')
            Rhino.RhinoApp.RunScript(script, False)
        elif kind == 'layer':
            layer = Rhino.DocObjects.Layer()
            try:
                layer.Name = 'Last layer %s %d' % (operation['id'], number)
                index = document.Layers.Add(layer)
                if index < 0: raise ValueError('last-selection layer insertion failed')
                state['layers'].append(index)
            finally:
                layer.Dispose()
        else:
            if not Rhino.RhinoApp.RunScript({'undo':'_Undo', 'redo':'_Redo', 'delete':'_Delete'}[kind], False):
                raise ValueError('last-selection command failed: ' + kind)
        records.append(last_snapshot(ids))
    return records

def on_idle(sender, event):
    # RunScript/redraw can pump messages and re-enter Idle during setup/Move.
    if state['busy'] or state['finished']: return
    state['busy'] = True
    try:
        if state['index'] == len(request['operations']):
            finish()
            return
        operation = request['operations'][state['index']]
        if state['stage'] == 'setup':
            document.Objects.UnselectAll()
            for i in range(3):
                state['ids'].append(document.Objects.AddLine(Rhino.Geometry.Point3d(i*5,0,0), Rhino.Geometry.Point3d(i*5,2,0)))
            ids = state['ids']
            for members in operation['groups']:
                state['groups'].append(document.Groups.Add([ids[i] for i in members]))
            if operation.get('reverse_bridge'):
                attributes = document.Objects.FindId(ids[1]).Attributes.Duplicate()
                try:
                    groups = list(attributes.GetGroupList() or [])
                    attributes.RemoveFromAllGroups()
                    for group in reversed(groups): attributes.AddToGroup(group)
                    document.Objects.ModifyAttributes(ids[1], attributes, True)
                finally:
                    attributes.Dispose()
            for i in operation.get('locked', []):
                if not document.Objects.Lock(ids[i], True): raise ValueError('lock failed')
            for i in operation.get('hidden', []):
                if not document.Objects.Hide(ids[i], True): raise ValueError('hide failed')
            if operation.get('layer_mode'):
                layer = Rhino.DocObjects.Layer()
                try:
                    layer.Name = 'Private bridge layer ' + operation['id']
                    index = document.Layers.Add(layer)
                    if index < 0: raise ValueError('layer insertion failed')
                    state['layers'].append(index)
                finally:
                    layer.Dispose()
                attrs = document.Objects.FindId(ids[1]).Attributes.Duplicate()
                try:
                    attrs.LayerIndex = index
                    document.Objects.ModifyAttributes(ids[1], attrs, True)
                finally:
                    attrs.Dispose()
                layer = document.Layers[index]
                layer.IsLocked = operation['layer_mode'] == 'locked'
                layer.IsVisible = operation['layer_mode'] != 'hidden'
                layer.CommitChanges()
                if document.Layers[index].IsLocked != (operation['layer_mode'] == 'locked') or document.Layers[index].IsVisible != (operation['layer_mode'] != 'hidden'):
                    raise ValueError('layer update failed')
            Rhino.RhinoApp.RunScript('_SetView _World _Top', False)
            Rhino.RhinoApp.RunScript('_Zoom _Extents', False)
            view = document.Views.ActiveView
            document.Views.Redraw()
            point = view.ActiveViewport.WorldToClient(Rhino.Geometry.Point3d(operation['seed']*5,1,0))
            screen = view.ClientToScreen(System.Drawing.Point(int(point.X), int(point.Y)))
            state['stage'] = 'wait'
            progress('PICK %s %d %d' % (operation['id'], screen.X, screen.Y), required=True)
        elif state['stage'] == 'wait':
            ack = os.path.join(root, 'click-ack.json')
            if not os.path.isfile(ack): return
            with open(ack) as stream: clicked = json.load(stream)
            if clicked != operation['id']: return
            state['stage'] = 'record'
            state['ready'] = time.time() + 0.5
        elif time.time() >= state['ready']:
            ids = state['ids']
            value = dict(selected=[i for i,key in enumerate(ids) if document.Objects.FindId(key).IsSelected(False)], modes=[str(document.Objects.FindId(key).Attributes.Mode) for key in ids], layers=[dict(visible=document.Layers[document.Objects.FindId(key).Attributes.LayerIndex].IsVisible, locked=document.Layers[document.Objects.FindId(key).Attributes.LayerIndex].IsLocked) for key in ids])
            if operation.get('move'):
                value['move_succeeded'] = bool(Rhino.RhinoApp.RunScript('_Move w0,0,0 w0,1,0', False)) if value['selected'] else None
                value['points'] = [[float(document.Objects.FindId(key).Geometry.PointAtStart.X), float(document.Objects.FindId(key).Geometry.PointAtStart.Y), float(document.Objects.FindId(key).Geometry.PointAtStart.Z)] for key in ids]
                # Read actual post-command states; do not assume Move retained them.
                value['modes'] = [str(document.Objects.FindId(key).Attributes.Mode) for key in ids]
                value['layers'] = [dict(visible=document.Layers[document.Objects.FindId(key).Attributes.LayerIndex].IsVisible, locked=document.Layers[document.Objects.FindId(key).Attributes.LayerIndex].IsLocked) for key in ids]
            if operation.get('recall_previous'):
                Rhino.RhinoApp.RunScript('_SelNone', False)
                Rhino.RhinoApp.RunScript('_-SelPrev _DeselectOthersBeforeSelect=_Yes _Enter', False)
                value['selected'] = [i for i,key in enumerate(ids) if document.Objects.FindId(key).IsSelected(False)]
            if operation.get('recall_last'):
                Rhino.RhinoApp.RunScript('_SelNone', False)
                Rhino.RhinoApp.RunScript('_-SelLast _DeselectOthersBeforeSelect=_Yes _Enter', False)
                value['selected'] = [i for i,key in enumerate(ids) if document.Objects.FindId(key).IsSelected(False)]
            if operation.get('last_steps'):
                value['last_states'] = last_sequence(operation, ids)
            state['results'].append(dict(id=operation['id'], value=value, elapsed_ns=0))
            errors = cleanup()
            if errors: raise RuntimeError('; '.join(errors))
            state['index'] += 1
            state['stage'] = 'setup'
    except Exception as error:
        finish(str(error))
    finally:
        state['busy'] = False

progress('worker: started')
Rhino.RhinoApp.Idle += on_idle
