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
state = dict(index=0, stage='setup', ids=[], groups=[], layers=[], results=[])

def progress(message):
    with open(os.path.join(root, 'worker-progress.log'), 'a') as stream: stream.write(message + '\n')

def cleanup():
    for index in state['layers']:
        layer = document.Layers[index]
        layer.IsLocked = False
        layer.IsVisible = True
        layer.CommitChanges()
    for key in state['ids']:
        document.Objects.Unlock(key, True)
        document.Objects.Show(key, True)
        document.Objects.Delete(key, True)
    for group in state['groups']: document.Groups.Delete(group)
    for layer in state['layers']: document.Layers.Delete(layer, True)
    state['layers'] = []
    state['ids'], state['groups'] = [], []

def finish(error=None):
    Rhino.RhinoApp.Idle -= on_idle
    cleanup()
    response = dict(protocol_version=1, engine='rhino', engine_version=str(Rhino.RhinoApp.Version), iterations=1, results=state['results'])
    if error: response['error'] = error
    temporary = os.path.join(root, 'response.json.tmp')
    with open(temporary, 'w') as stream: json.dump(response, stream)
    os.rename(temporary, os.path.join(root, 'response.json'))
    progress('worker: response published')
    document.Modified = False
    Rhino.RhinoApp.RunScript('_Exit _No', False)

def on_idle(sender, event):
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
                groups = list(attributes.GetGroupList() or [])
                attributes.RemoveFromAllGroups()
                for group in reversed(groups): attributes.AddToGroup(group)
                document.Objects.ModifyAttributes(ids[1], attributes, True)
                attributes.Dispose()
            for i in operation.get('locked', []):
                if not document.Objects.Lock(ids[i], True): raise ValueError('lock failed')
            for i in operation.get('hidden', []):
                if not document.Objects.Hide(ids[i], True): raise ValueError('hide failed')
            if operation.get('layer_mode'):
                layer = Rhino.DocObjects.Layer()
                layer.Name = 'Private bridge layer ' + operation['id']
                index = document.Layers.Add(layer)
                if index < 0: raise ValueError('layer insertion failed')
                layer.Dispose()
                state['layers'].append(index)
                attrs = document.Objects.FindId(ids[1]).Attributes.Duplicate()
                attrs.LayerIndex = index
                document.Objects.ModifyAttributes(ids[1], attrs, True)
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
            progress('PICK %s %d %d' % (operation['id'], screen.X, screen.Y))
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
            state['results'].append(dict(id=operation['id'], value=value, elapsed_ns=0))
            cleanup()
            state['index'] += 1
            state['stage'] = 'setup'
    except Exception as error:
        finish(str(error))

progress('worker: started')
Rhino.RhinoApp.Idle += on_idle
