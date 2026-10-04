"""Fixed ScaleNU command recipes on an owned empty document and private display."""
import re

SOURCES = ('point', 'rational', 'periodic', 'surface', 'mesh')
PLANES = ('world', 'rotated', 'tilted')
INPUTS = {
    'numeric': 'w1,2,3 2 3 .5',
    'negative': 'w1,2,3 2 -1 .5',
    'zero': 'w0,0,0 0 1 1',
    'references': 'w0,0,0 w1,1,1 w3,4,0 1 1',
    'defaults': 'w0,0,0 _Enter _Enter _Enter',
    'world': '_WorldCoordinates w1,2,3 2 3 .5',
    'ref_x': 'w0,0,0 w2,0,0 w6,0,0 1 1',
    'ref_y': 'w0,0,0 1 w0,2,0 w0,6,0 1',
    'ref_z': 'w0,0,0 1 1 w0,0,2 w0,0,-6',
    'ref_distance': 'w0,0,0 w2,0,0 6 w6,0,0 1 1',
    'ref_y_distance': 'w0,0,0 1 w0,2,0 6 w0,6,0 1',
    'ref_z_distance': 'w0,0,0 1 1 w0,0,2 6 w0,0,-6',
    'ref_offaxis_distance': 'w0,0,0 w1,1,1 6 w3,4,0 1 1',
    'repeat': 'w1,2,3 2 3 .5 4 .5 1',
}


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'source', 'plane', 'input', 'copy', 'parent', 'point', 'selection', 'view'}
            or op['op'] != 'scale_nu' or not isinstance(op['id'], str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
            or op['source'] not in SOURCES or op['plane'] not in PLANES
            or not isinstance(op['input'], str) or op['input'] not in INPUTS
            or any(type(op[k]) is not bool for k in ('copy', 'parent', 'point'))
            or op['selection'] not in ('auto', 'all', 'object')):
        raise ValueError('invalid bounded ScaleNU recipe')
    if op['selection'] == 'all' and (op['parent'] or op['point']):
        raise ValueError('ScaleNU source macro does not match selection')
    if op['view'] not in ('Top', 'Perspective') or (op['input'] == 'repeat' and not op['copy']):
        raise ValueError('invalid ScaleNU view or copy continuation')


def request():
    rows = [('point', p, 'numeric', False, False, False, 'auto') for p in PLANES]
    rows += [('point', 'world', i, False, False, False, 'auto') for i in ('negative', 'zero', 'references', 'defaults')]
    rows += [('point', 'tilted', 'world', False, False, False, 'auto'),
             ('rational', 'tilted', 'numeric', True, True, True, 'auto'),
             ('mesh', 'world', 'zero', False, False, False, 'auto')]
    rows += [('point', 'world', i, False, False, False, 'auto') for i in ('ref_x', 'ref_y', 'ref_z', 'ref_distance')]
    rows += [('periodic', 'rotated', 'numeric', False, False, False, 'auto'),
             ('surface', 'tilted', 'numeric', True, False, False, 'auto'),
             ('mesh', 'world', 'negative', True, False, False, 'auto'),
             ('rational', 'world', 'repeat', True, False, False, 'auto'),
             ('rational', 'world', 'numeric', False, False, False, 'all'),
             ('point', 'rotated', 'numeric', False, False, False, 'all'),
             ('point', 'world', 'references', False, False, False, 'auto'),
             ('mesh', 'world', 'zero', False, False, False, 'object'),
             ('point', 'world', 'ref_x', False, False, False, 'auto')]
    rows += [('point', 'world', i, False, False, False, 'auto') for i in ('ref_y_distance', 'ref_z_distance', 'ref_offaxis_distance')]
    operations = [dict(op='scale_nu', id='scale-nu-'+str(i), source=s, plane=p, input=t,
                       copy=c, parent=a, point=b, selection=k, view='Perspective' if i==20 else 'Top') for i, (s, p, t, c, a, b, k) in enumerate(rows)]
    return dict(protocol_version=1, iterations=1, operations=operations)


def run(op, host):
    validate(op)
    Rhino, System = host['Rhino'], host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('ScaleNU history requires idle execution')
    if list(doc.Objects):
        raise ValueError('ScaleNU requires an empty owned document')
    from grip_transform_probe import create_source
    from join_probe import observe_command
    view = doc.Views.ActiveView
    original_plane = view.ActiveViewport.GetConstructionPlane()
    original_view = Rhino.DocObjects.ViewportInfo(view.ActiveViewport)
    source_id, peer_id = None, None
    plane = Rhino.Geometry.Plane.WorldXY
    if op['plane'] == 'rotated':
        plane = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]), Rhino.Geometry.Vector3d(0.,1.,0.), Rhino.Geometry.Vector3d(-1.,0.,0.))
    elif op['plane'] == 'tilted':
        plane = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]), Rhino.Geometry.Vector3d(1.,1.,0.), Rhino.Geometry.Vector3d(-1.,1.,2.))

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber):
            geometry = obj.Geometry
            row = dict(role='source' if obj.Id == source_id else 'point' if obj.Id == peer_id else 'output',
                       selected=bool(obj.IsSelected(False)), grips_on=bool(obj.GripsOn),
                       grips=[dict(index=int(g.Index), point=host['_xyz'](g.CurrentLocation), selected=bool(g.IsSelected(False)))
                              for g in (obj.GetGrips() or [])], kind=geometry.GetType().Name, name=obj.Attributes.Name)
            if isinstance(geometry, Rhino.Geometry.Curve):
                row['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
            elif isinstance(geometry, Rhino.Geometry.Brep):
                row['surface'] = host['_nurbs_surface_definition'](geometry.Surfaces[0].ToNurbsSurface())
            elif isinstance(geometry, Rhino.Geometry.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](geometry)
            elif isinstance(geometry, Rhino.Geometry.Point):
                row['point'] = host['_xyz'](geometry.Location)
            else:
                raise ValueError('unexpected ScaleNU output')
            rows.append(row)
        return rows

    try:
        if not view.ActiveViewport.SetProjection(getattr(Rhino.Display.DefinedViewportProjection, op['view']), 'Owned ScaleNU probe', False):
            raise ValueError('ScaleNU view setup failed')
        view.ActiveViewport.SetConstructionPlane(plane)
        # Seed the command's three remembered factors through public input.
        # The reset object is removed before the source Undo record begins.
        seed = [2.,3.,.5] if op['input'] == 'defaults' else [1.,1.,1.]
        reset = doc.Objects.AddPoint(host['_point']([1.,1.,1.]))
        doc.Objects.Select(reset)
        if not Rhino.RhinoApp.RunScript('_ScaleNU _Copy=_No w0,0,0 '+ ' '.join(str(f) for f in seed), False):
            raise ValueError('ScaleNU default seeding failed')
        doc.Objects.Delete(reset, True)
        serial = doc.BeginUndoRecord('ScaleNU sources')
        try:
            attributes = Rhino.DocObjects.ObjectAttributes()
            attributes.Name = 'grip source'
            geometry = Rhino.Geometry.Point(host['_point']([2.,3.,4.])) if op['source'] == 'point' else create_source(op['source'], host)
            source_id = doc.Objects.Add(geometry, attributes)
            peer_id = doc.Objects.AddPoint(host['_point']([8.,0.,0.])) if op['point'] else None
            if source_id == System.Guid.Empty or peer_id == System.Guid.Empty:
                raise ValueError('ScaleNU source insertion failed')
        finally:
            doc.EndUndoRecord(serial)
        source = doc.Objects.FindId(source_id)
        if op['source'] == 'point' or op['selection'] == 'object':
            if op['selection'] != 'all':
                doc.Objects.Select(source_id)
        else:
            source.GripsOn = True
            if op['selection'] == 'auto':
                for index in (0, 2):
                    source.GetGrips()[index].Select(True)
        if op['parent']:
            doc.Objects.Select(source_id)
        if op['point']:
            doc.Objects.Select(peer_id)
        before = snapshot()
        camera_before = dict(location=host['_xyz'](view.ActiveViewport.CameraLocation), direction=host['_xyz'](view.ActiveViewport.CameraDirection))
        macro = '_ScaleNU '+('_SelAll _Enter ' if op['selection'] == 'all' else '')+'_Copy=_'+('Yes ' if op['copy'] else 'No ')+INPUTS[op['input']]
        if op['copy']:
            macro += ' _Enter'
        host['_record_progress'](op['id']+' '+macro)
        history_before = Rhino.RhinoApp.CommandHistoryWindowText
        success, after, events = observe_command(Rhino.Commands.Command, 'ScaleNU',
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        after_script = snapshot()
        history = Rhino.RhinoApp.CommandHistoryWindowText[len(history_before):]
        Rhino.RhinoApp.RunScript('_Undo', False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo', False)
        redo = snapshot()
        return dict(plane=dict(origin=host['_xyz'](plane.Origin), x_axis=host['_xyz'](plane.XAxis), y_axis=host['_xyz'](plane.YAxis)),
                    camera_before=camera_before, camera=dict(location=host['_xyz'](view.ActiveViewport.CameraLocation), direction=host['_xyz'](view.ActiveViewport.CameraDirection)),
                    seed=seed, history=history,
                    before=before, after=after, after_script=after_script, undo=undo, redo=redo,
                    success=success, events=events, macro=macro), 0
    finally:
        view.ActiveViewport.SetViewProjection(original_view, False)
        view.ActiveViewport.SetConstructionPlane(original_plane)
        for obj in list(doc.Objects):
            if obj.GripsOn:
                obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
