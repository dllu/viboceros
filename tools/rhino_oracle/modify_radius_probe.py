"""Owned, public ModifyRadius command observations and source artifacts."""
import math
import re

FAMILIES = ('arc', 'circle', 'nurbs_arc', 'nurbs_circle', 'near_circle', 'polycurve', 'tilted', 'reversed')
MODES = ('Radius', 'Diameter', 'Area', 'Circumference')


def validate(op):
    required = {'op', 'id', 'family', 'mode', 'value', 'artifact_path'}
    optional = {'copy', 'preselect', 'domain', 'copies', 'mixed', 'delta'}
    if (not isinstance(op, dict) or not required <= set(op) or set(op)-required-optional
            or op['op'] != 'modify_radius_command' or op['family'] not in FAMILIES
            or op['mode'] not in MODES or not isinstance(op['id'], str)
            or re.match(r'^[A-Za-z0-9_-]{1,100}\Z', op['id']) is None
            or not isinstance(op['artifact_path'], str) or not op['artifact_path'].endswith('.3dm')):
        raise ValueError('invalid ModifyRadius recipe')
    if type(op['value']) not in (int, float) or not 0 < op['value'] <= 1e6:
        raise ValueError('invalid bounded ModifyRadius size')
    for key in ('copy', 'preselect', 'mixed'):
        if type(op.get(key, False)) is not bool:
            raise ValueError('invalid ModifyRadius input flag')
    if type(op.get('copies', 1)) is not int or not 1 <= op.get('copies', 1) <= 3:
        raise ValueError('invalid bounded ModifyRadius count')
    if 'domain' in op:
        d = op['domain']
        if (not isinstance(d, list) or len(d) != 2
                or any(type(x) not in (int, float) or math.isnan(x) or math.isinf(x) for x in d)
                or not d[0] < d[1]):
            raise ValueError('invalid ModifyRadius domain')
    d = op.get('delta', 1e-8)
    if type(d) not in (int, float) or not 0 <= d <= .1:
        raise ValueError('invalid ModifyRadius perturbation')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('ModifyRadius capture requires one iteration')
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if R.Commands.Command.InCommand() or list(doc.Objects):
        raise ValueError('ModifyRadius requires an owned empty idle document')
    owned, attributes, layers, groups = [], [], [], []
    saved_layer = doc.Layers.CurrentLayerIndex

    def keep(curve):
        owned.append(curve)
        if curve is None or not curve.IsValid:
            raise ValueError('invalid ModifyRadius source')
        return curve

    def record(c):
        value = host['_interchange_curve_record'](c)
        if isinstance(c, G.NurbsCurve):
            value['rational'] = bool(c.IsRational)
        if isinstance(c, G.ArcCurve):
            a = c.Arc
            value['arc'] = dict(center=host['_xyz'](a.Center), x=host['_xyz'](a.Plane.XAxis),
                                normal=host['_xyz'](a.Plane.ZAxis), radius=float(a.Radius),
                                angles=[float(a.StartAngle), float(a.EndAngle)])
        if isinstance(c, G.PolyCurve):
            value['segments'] = [record(c.SegmentCurve(i)) for i in range(c.SegmentCount)]
        return value

    def strings(obj):
        data = obj.GetUserStrings()
        return dict((str(k), str(data[k])) for k in data.AllKeys) if data else {}

    def snapshot():
        return [dict(id=str(o.Id), name=str(o.Attributes.Name) if o.Attributes.Name else None,
                     layer=int(o.Attributes.LayerIndex), groups=list(int(i) for i in (o.Attributes.GetGroupList() or [])),
                     color_source=str(o.Attributes.ColorSource),
                     color=[int(o.Attributes.ObjectColor.R), int(o.Attributes.ObjectColor.G), int(o.Attributes.ObjectColor.B)],
                     user_text=strings(o.Attributes), geometry_text=strings(o.Geometry),
                     selected=bool(o.IsSelected(False)), geometry=record(o.Geometry)) for o in doc.Objects]

    try:
        family = op['family']
        plane = G.Plane(G.Point3d(3., -4., 5.), G.Vector3d(1., 2., 3.)) if family == 'tilted' else G.Plane(G.Point3d(3., -4., 5.), G.Vector3d.ZAxis)
        angle = 2.*math.pi if family in ('circle', 'nurbs_circle', 'near_circle') else .75*math.pi
        source = keep(G.ArcCurve(G.Arc(G.Circle(plane, 2.), G.Interval(.3, .3+angle))))
        if family in ('nurbs_arc', 'nurbs_circle', 'near_circle'):
            source = keep(source.ToNurbsCurve())
            if family == 'near_circle':
                cv = source.Points[1]
                source.Points.SetPoint(1, cv.Location + G.Vector3d(op.get('delta', 1e-8), 0., 0.), cv.Weight)
        if family == 'polycurve':
            first, second = source.Split(source.Domain.Mid)
            owned.extend([first, second])
            source = G.PolyCurve(); owned.append(source)
            if not source.Append(first) or not source.Append(second):
                raise ValueError('failed to create polycurve')
        if family == 'reversed':
            source.Reverse()
        if 'domain' in op:
            source.Domain = G.Interval(*op['domain'])
        source.SetUserString('GeometryCode', 'source')
        layer = R.DocObjects.Layer(); layer.Name = 'ModifyRadius_'+str(S.Guid.NewGuid())
        try:
            layer_index = doc.Layers.Add(layer)
        finally:
            layer.Dispose()
        if layer_index < 0:
            raise ValueError('source layer creation failed')
        layers.append(layer_index)
        ids = []
        for index in range(op.get('copies', 1)):
            a = R.DocObjects.ObjectAttributes(); attributes.append(a)
            a.Name = 'RadiusSource'+str(index); a.LayerIndex = layer_index
            a.SetUserString('ObjectCode', 'source'); a.ObjectColor = S.Drawing.Color.FromArgb(17, 47, 87)
            a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
            placed = keep(source.DuplicateCurve())
            if index:
                placed.Transform(G.Transform.Translation(20.*index, 0., 0.))
            key = doc.Objects.AddCurve(placed, a)
            if key == S.Guid.Empty:
                raise ValueError('source insertion failed')
            ids.append(key)
        if family == 'near_circle':
            a = R.DocObjects.ObjectAttributes(); attributes.append(a)
            a.Name = 'RadiusWitness'; a.LayerIndex = layer_index
            a.SetUserString('ObjectCode', 'source'); a.ObjectColor = S.Drawing.Color.FromArgb(17, 47, 87)
            a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
            witness = keep(G.ArcCurve(G.Circle(G.Plane.WorldXY, 2.)))
            witness.SetUserString('GeometryCode', 'source')
            ids.append(doc.Objects.AddCurve(witness, a))
        source_ids = list(ids)
        if op.get('mixed'):
            a = R.DocObjects.ObjectAttributes(); attributes.append(a); a.Name = 'IneligibleLine'
            ids.append(doc.Objects.AddLine(G.Point3d(0., 0., 0.), G.Point3d(1., 0., 0.), a))
        group = doc.Groups.Add('RadiusGroup_'+str(S.Guid.NewGuid()), S.Array[S.Guid](ids)); groups.append(group)
        if group < 0:
            raise ValueError('source group creation failed')
        if op.get('preselect', True):
            for key in ids:
                doc.Objects.Select(key)
        path = op['artifact_path']; path = 'Z:'+path.replace('/', '\\') if path.startswith('/') else path
        options = R.FileIO.FileWriteOptions(); options.FileVersion = 8
        options.SuppressDialogBoxes = True; options.SuppressAllInput = True; options.WriteUserData = True; options.UpdateDocumentPath = False
        try:
            if not doc.Write3dmFile(path, options):
                raise ValueError('source artifact write failed')
        finally:
            options.Dispose()
        doc.ClearUndoRecords(True); before = snapshot()
        marker = 'Viboceros ModifyRadius '+str(S.Guid.NewGuid()); R.RhinoApp.WriteLine(marker)
        macro = '_ModifyRadius '
        if not op.get('preselect', True):
            macro += ' '.join('_SelID '+str(key) for key in source_ids)+' _Enter '
        macro += '_Copy=_%s ' % ('Yes' if op.get('copy', False) else 'No')
        if op['mode'] != 'Radius':
            macro += '_'+op['mode']+' '
        macro += repr(op['value'])
        success, after, events = observe_command(R.Commands.Command, 'ModifyRadius',
            lambda: R.RhinoApp.RunScript(macro, False), snapshot,
            lambda: [str(o.Id) for o in doc.Objects if o.IsSelected(False)], True)
        command_result = [e['result'] for e in events if e['name'] == 'ModifyRadius'][0]
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1]
        changed = before != after
        undo = redo = None
        if changed:
            R.RhinoApp.RunScript('_Undo', False); undo = snapshot()
            R.RhinoApp.RunScript('_Redo', False); redo = snapshot()
        return dict(success=success, source_ids=[str(k) for k in source_ids], before=before,
                    after=after, undo=undo, redo=redo, changed=changed, macro=macro,
                    history=history, events=events, command_result=command_result, recipe=op), 0
    finally:
        if R.Commands.Command.InCommand():
            R.RhinoApp.RunScript('!', False)
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        for index in groups:
            if index >= 0 and not doc.Groups.IsDeleted(index):
                doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer, True)
        for index in layers:
            doc.Layers.Delete(index, True)
        for a in reversed(attributes):
            a.Dispose()
        for curve in reversed(owned):
            curve.Dispose()
