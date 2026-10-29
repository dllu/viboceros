"""Observe public MergeFaces on owned SubDs, including component identities."""
import math
import re


def validate(op):
    fields = {'op', 'id', 'vertices', 'faces', 'components', 'corners', 'creases', 'soft'}
    if (not isinstance(op, dict) or set(op) != fields or op['op'] != 'subd_merge_faces'
            or not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None):
        raise ValueError('fixed bounded SubD merge recipe required')
    vertices, faces = op['vertices'], op['faces']
    if (not isinstance(vertices, list) or not 3 <= len(vertices) <= 128
            or not isinstance(faces, list) or not 1 <= len(faces) <= 128):
        raise ValueError('bounded SubD source required')
    if any(not isinstance(p, list) or len(p) != 3 or any(type(x) not in (int, float)
           or math.isnan(x) or math.isinf(x) for x in p) for p in vertices):
        raise ValueError('finite control points required')
    if any(not isinstance(f, list) or len(f) not in (3, 4)
           or any(type(i) is not int or not 0 <= i < len(vertices) for i in f)
           or len(set(f)) != len(f) for f in faces):
        raise ValueError('valid triangle or quad source required')
    def pair(value):
        return (isinstance(value, list) and len(value) == 2 and all(type(i) is int
                and 0 <= i < len(vertices) for i in value) and value[0] != value[1])
    components = op['components']
    if not isinstance(components, list) or len(components) > 128:
        raise ValueError('bounded component selection required')
    for c in components:
        if not isinstance(c, dict) or c.get('kind') not in ('face', 'edge', 'vertex'):
            raise ValueError('typed SubD component required')
        if c['kind'] == 'edge':
            if set(c) != {'kind', 'vertices'} or not pair(c['vertices']):
                raise ValueError('edge endpoint pair required')
        elif (set(c) != {'kind', 'index'} or type(c['index']) is not int
              or not 0 <= c['index'] < (len(faces) if c['kind'] == 'face' else len(vertices))):
            raise ValueError('bounded component ordinal required')
    if (not isinstance(op['corners'], list) or len(op['corners']) > len(vertices)
            or any(type(i) is not int or not 0 <= i < len(vertices) for i in op['corners'])
            or not isinstance(op['creases'], list) or len(op['creases']) > 128
            or any(not pair(p) or p != sorted(p) for p in op['creases'])):
        raise ValueError('bounded corner and crease seeds required')
    soft = op['soft']
    if soft is not None and (not isinstance(soft, dict) or set(soft) != {'weight', 'edges'}
            or type(soft['weight']) not in (int, float) or not 0 <= soft['weight'] <= 100
            or not isinstance(soft['edges'], list) or not 1 <= len(soft['edges']) <= 128
            or any(not pair(p) or p != sorted(p) for p in soft['edges'])):
        raise ValueError('bounded soft crease seed required')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1 or host['Rhino'].Commands.Command.InCommand():
        raise ValueError('one idle owned workflow required')
    from subd_edit_scene import Scene
    from hide_pt_input import run as drive
    definition = dict(vertices=op['vertices'], faces=op['faces'], corners=op['corners'],
                      creases=op['creases'])
    if op['soft'] is not None:
        definition.update(soft=op['soft']['weight'], edges=op['soft']['edges'])
    with Scene(dict(sources=[definition]), host) as scene:
        R = scene.R
        def snapshot():
            rows = scene.snapshot()
            objects = sorted(list(scene.doc.Objects), key=lambda o: o.RuntimeSerialNumber)
            for row, obj in zip(rows, objects):
                if 'subd' not in row:
                    continue
                g = obj.Geometry
                row['subd'].update(vertex_ids=[int(v.Id) for v in scene.vertices(g)],
                                   edge_ids=[int(e.Id) for e in g.Edges],
                                   face_ids=[int(f.Id) for f in g.Faces])
                row['selected_faces'] = [int(f.Id) for f in g.Faces
                                         if obj.IsSubObjectSelected(f.ComponentIndex())]
            return rows
        obj = scene.doc.Objects.FindId(scene.ids[0])
        vertices, faces = scene.vertices(obj.Geometry), list(obj.Geometry.Faces)
        ordinals = {int(v.Id): i for i, v in enumerate(vertices)}
        selected = []
        for c in op['components']:
            if c['kind'] == 'edge':
                pair = sorted(c['vertices'])
                matches = [e for e in obj.Geometry.Edges if sorted([ordinals[int(e.VertexFrom.Id)],
                           ordinals[int(e.VertexTo.Id)]]) == pair]
                if len(matches) != 1:
                    raise ValueError('selected endpoints do not identify an edge')
                component = matches[0]
            else:
                component = (faces if c['kind'] == 'face' else vertices)[c['index']]
            if not obj.SelectSubObject(component.ComponentIndex(), True, True, False):
                raise ValueError('owned SubD component selection failed')
            selected.append(dict(kind=c['kind'], id=int(component.Id)))
        before, source = snapshot(), scene.archive()
        events = []
        def ended(sender, event):
            if event.CommandEnglishName == 'MergeFaces':
                events.append(str(event.CommandResult))
        marker = 'Owned SubD MergeFaces ' + str(scene.S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        R.Commands.Command.EndCommand += ended
        try:
            _, timer_fired, _ = drive(host, command='MergeFaces', cancel=True)
        finally:
            R.Commands.Command.EndCommand -= ended
        if len(events) != 1:
            raise ValueError('MergeFaces did not finish: ' + str(events))
        after, after_archive = snapshot(), scene.archive()
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1]
        R.RhinoApp.RunScript('_Undo', False)
        undone = snapshot()
        R.RhinoApp.RunScript('_Redo', False)
        redone = snapshot()
        return dict(before=before, after=after, undone=undone, redone=redone,
                    source_3dm=source, after_3dm=after_archive, selected_input=selected,
                    command_result=events[0], input_timer_fired=timer_fired, history=history), 0
