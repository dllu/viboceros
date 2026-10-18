"""Owned RemapCPlane commands through public construction-plane APIs."""
import re

WORLD = ('Top', 'Bottom', 'Front', 'Back', 'Right', 'Left')
SOURCES = ('points', 'rational', 'surface', 'mesh')


def validate(op):
    if (not isinstance(op, dict)
            or not {'op', 'id', 'source_plane', 'target', 'mode', 'source', 'copy', 'postselected', 'group', 'grips'} <= set(op)
            or set(op) - {'op', 'id', 'source_plane', 'target', 'mode', 'source', 'copy', 'postselected', 'group', 'grips', 'retry_target', 'setup_copy'}
            or op['op'] != 'remap_cplane'
            or not isinstance(op['id'], str) or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
            or op['source_plane'] not in ('Top', 'tilted')
            or op['target'] not in WORLD + ('Owned target',)
            or op['mode'] not in ('CPlane', 'View') or op['source'] not in SOURCES
            or any(type(op[k]) is not bool for k in ('copy', 'postselected', 'group', 'grips'))
            or op['grips'] and (op['source'] == 'points' or op['postselected'])
            or 'retry_target' in op and op['retry_target'] not in WORLD
            or 'setup_copy' in op and (type(op['setup_copy']) is not bool or op['grips'] or op['postselected'])):
        raise ValueError('invalid bounded RemapCPlane recipe')


def validate_capture(op):
    """A live macro must finish, including any rejected-target name getter."""
    validate(op)
    same = op['source_plane'] == 'Top' and op['target'] == 'Top'
    if (same and 'retry_target' not in op
            or 'retry_target' in op and (not same or op['retry_target'] == 'Top')):
        raise ValueError('RemapCPlane capture needs one rejected target followed by a distinct retry')


def request():
    rows = [dict(source_plane='tilted', target=t, mode='CPlane', source='points',
                 copy=c, postselected=False, group=True, grips=False)
            for t in WORLD + ('Owned target',) for c in (False, True)]
    rows += [dict(source_plane='Top', target='Owned target', mode=m, source=s,
                  copy=c, postselected=p, group=True, grips=False)
             for m, s in [('CPlane', 'rational'), ('CPlane', 'surface'), ('CPlane', 'mesh'), ('View', 'points')]
             for c in (False, True) for p in (False, True)]
    rows += [dict(source_plane='tilted', target='Front', mode='CPlane', source=s,
                  copy=c, postselected=False, group=False, grips=True)
             for s in ('rational', 'surface', 'mesh') for c in (False, True)]
    rows += [dict(source_plane='Top', target='Top', mode='CPlane', source='points',
                  copy=c, postselected=False, group=False, grips=False, retry_target='Front') for c in (False, True)]
    operations = [dict(row, op='remap_cplane', id='remap-cplane-'+str(i)) for i, row in enumerate(rows)]
    for op in operations:
        validate(op)
    return dict(protocol_version=1, iterations=1, operations=operations)


def run(op, host):
    validate_capture(op)
    R, System = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if R.Commands.Command.InCommand() or list(doc.Objects):
        raise ValueError('RemapCPlane requires idle execution in an empty owned document')
    from grip_transform_probe import create_source
    from join_probe import observe_command
    vp = doc.Views.ActiveView.ActiveViewport
    old_plane = vp.GetConstructionPlane().Plane
    source_ids, named_index, named_view_index = [], -1, -1
    def plane(origin, x, y):
        return G.Plane(host['_point'](origin), G.Vector3d(*x), G.Vector3d(*y))
    source_plane = G.Plane.WorldXY if op['source_plane'] == 'Top' else plane([5., -4., 2.], [1., 1., 0.], [-1., 1., 2.])
    target = plane([-3., 7., 11.], [0., 1., 1.], [2., 1., -1.])
    def frame(p):
        return dict(origin=host['_xyz'](p.Origin), x_axis=host['_xyz'](p.XAxis), y_axis=host['_xyz'](p.YAxis))
    def snapshot():
        groups = sorted([g for g in doc.Groups if not g.IsDeleted], key=lambda g: g.Index)
        group_indices = [g.Index for g in groups]
        objects = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            geometry = obj.Geometry
            row = dict(source=source_ids.index(obj.Id) if obj.Id in source_ids else None,
                       selected=bool(obj.IsSelected(False)), name=obj.Attributes.Name,
                       groups=[group_indices.index(i) for i in (obj.Attributes.GetGroupList() or [])],
                       grips_on=bool(obj.GripsOn),
                       grips=[dict(index=int(g.Index), point=host['_xyz'](g.CurrentLocation), selected=bool(g.IsSelected(False)))
                              for g in (obj.GetGrips() or [])])
            if isinstance(geometry, G.PointCloud):
                row['points'] = [host['_xyz'](p.Location) for p in geometry]
            elif isinstance(geometry, G.Curve):
                row['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
            elif isinstance(geometry, G.Brep):
                row['surface'] = host['_nurbs_surface_definition'](geometry.Surfaces[0].ToNurbsSurface())
            elif isinstance(geometry, G.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](geometry)
            else:
                raise ValueError('unexpected RemapCPlane output')
            objects.append(row)
        return dict(objects=objects, groups=[dict(name=g.Name, count=doc.Groups.GroupObjectCount(g.Index)) for g in groups],
                    plane=frame(vp.GetConstructionPlane().Plane))
    try:
        if op['target'] == 'Owned target':
            if op['mode'] == 'CPlane':
                named_index = doc.NamedConstructionPlanes.Add(op['target'], target)
                if named_index < 0:
                    raise ValueError('cannot create owned target CPlane')
            else:
                vp.SetConstructionPlane(target)
                named_view_index = doc.NamedViews.Add(op['target'], vp.Id)
                if named_view_index < 0:
                    raise ValueError('cannot create owned target view')
        vp.SetConstructionPlane(source_plane)
        serial = doc.BeginUndoRecord('RemapCPlane sources')
        try:
            for i in range(2 if op['source'] == 'points' else 1):
                geometry = G.PointCloud([host['_point'](p) for p in [[1.+3*i, 2., 3.], [2.+3*i, 2., 3.], [1.+3*i, 3., 3.], [1.+3*i, 2., 4.]]]) if op['source'] == 'points' else create_source(op['source'], host)
                attributes = R.DocObjects.ObjectAttributes()
                attributes.Name = 'remap source '+str(i)
                try:
                    object_id = doc.Objects.Add(geometry, attributes)
                finally:
                    geometry.Dispose()
                if object_id == System.Guid.Empty:
                    raise ValueError('cannot insert RemapCPlane source')
                source_ids.append(object_id)
            if op['group']:
                doc.Groups.Add('Owned remap group', source_ids)
        finally:
            doc.EndUndoRecord(serial)
        if op['grips']:
            obj = doc.Objects.FindId(source_ids[0])
            obj.GripsOn = True
            for index in (0, 2):
                obj.GetGrips()[index].Select(True)
        elif not op['postselected']:
            for object_id in source_ids:
                doc.Objects.Select(object_id)
        doc.ClearUndoRecords(True)
        setup = None
        if 'setup_copy' in op:
            setup_macro = '_RemapCPlane _Copy=_'+('Yes' if op['setup_copy'] else 'No')+' _Cancel'
            host['_record_progress'](op['id']+' setup '+setup_macro)
            setup_success, setup_after, setup_events = observe_command(R.Commands.Command, 'RemapCPlane',
                lambda: R.RhinoApp.RunScript(setup_macro, True), snapshot, lambda: [], True)
            setup = dict(success=setup_success, after=setup_after, events=setup_events, macro=setup_macro)
            for object_id in source_ids:
                doc.Objects.Select(object_id)
        before = snapshot()
        picks = ' '.join('_SelID '+str(i) for i in source_ids)+' _Enter ' if op['postselected'] else ''
        copy_option = '_Copy=_'+('Yes' if op['copy'] else 'No')+' ' if 'setup_copy' not in op else ''
        macro = '_RemapCPlane '+picks+copy_option+'_'+op['mode']+' "'+op['target']+'"'
        if 'retry_target' in op:
            macro += ' "'+op['retry_target']+'"'
        marker = 'Viboceros RemapCPlane '+str(System.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success, after, events = observe_command(R.Commands.Command, 'RemapCPlane',
            lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        if R.Commands.Command.InCommand():
            raise ValueError('RemapCPlane remained active')
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1].strip()
        after_script = snapshot()
        undo_returned = bool(R.RhinoApp.RunScript('_Undo', False))
        undo = snapshot()
        redo_returned = bool(R.RhinoApp.RunScript('_Redo', False))
        redo = snapshot()
        result = dict(success=success, macro=macro, history=history, before=before, after=after,
                    after_script=after_script, undo=undo, redo=redo,
                    undo_returned=undo_returned, redo_returned=redo_returned, events=events)
        if setup is not None: result['setup'] = setup
        return result, 0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:
                obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
        for group in list(doc.Groups):
            if not group.IsDeleted:
                doc.Groups.Delete(group.Index)
        if named_index >= 0:
            doc.NamedConstructionPlanes.Delete(named_index)
        if named_view_index >= 0:
            doc.NamedViews.Delete(named_view_index)
        vp.SetConstructionPlane(old_plane)
