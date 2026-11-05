"""Observe deletion and restoration of owned analyzed objects through native UI."""
if __package__:
    from .edge_analysis_lifetime_cases import validate
else:
    from edge_analysis_lifetime_cases import validate


def run(op, host, iterations, tolerance):
    validate(op)
    from edge_analysis_mixed_probe import source, geometry
    from join_probe import observe_command
    from owned_panel import inspect
    from edge_zoom_input import start
    R, S = host['Rhino'], host['System']
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    old, keys, stages, commands = doc.SubDAppearance, [], [], []
    finish_input = None
    kinds = ['brep', 'mesh', 'subd']
    try:
        R.RhinoApp.RunScript('_ShowEdgesOff', False)
        for kind in kinds:
            g = source(kind, host)
            attrs = R.DocObjects.ObjectAttributes()
            attrs.Name = kind
            try:
                key = doc.Objects.Add(g, attrs)
            finally:
                attrs.Dispose()
                g.Dispose()
            if key == S.Guid.Empty:
                raise ValueError('owned lifetime source admission failed')
            keys.append(key)
        doc.SubDAppearance = R.Geometry.SubDComponentLocation.ControlNet if op['flat'] else R.Geometry.SubDComponentLocation.Surface
        doc.ClearUndoRecords(True)
        model = R.FileIO.File3dm()
        try:
            for key in keys:
                g = doc.Objects.FindId(key).Geometry
                if isinstance(g, R.Geometry.Brep):model.Objects.AddBrep(g)
                elif isinstance(g, R.Geometry.Mesh):model.Objects.AddMesh(g)
                else:model.Objects.AddSubD(g)
            archive = S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()

        def snapshot():
            sources = []
            for kind, key in zip(kinds, keys):
                obj = doc.Objects.FindId(key)
                present = obj is not None and not obj.IsDeleted
                sources.append(dict(kind=kind, present=present,
                                    geometry=geometry(obj.Geometry, host) if present else None,
                                    selected=bool(obj.IsSelected(False)) if present else False))
            points = [o for o in doc.Objects if o.Id not in keys and isinstance(o.Geometry, R.Geometry.Point)]
            points.sort(key=lambda o: int(o.RuntimeSerialNumber))
            result = dict(sources=sources, objects=len(list(doc.Objects)),
                          points=[host['_xyz'](o.Geometry.Location) for o in points])
            if op.get('runtime'):
                result['runtime_order'] = [kind for serial, kind in runtime_serials()]
            return result

        def runtime_serials():
            return sorted((int(obj.RuntimeSerialNumber), kind) for kind, key in zip(kinds, keys)
                          for obj in [doc.Objects.FindId(key)] if obj is not None and not obj.IsDeleted)

        def stage(action):
            doc.Views.Redraw()
            R.RhinoApp.Wait()
            stages.append(dict(action=action, state=snapshot(), panel=inspect('Edge Analysis'),
                               runtime_serials=[dict(kind=kind, serial=serial) for serial, kind in runtime_serials()],
                               selection_states=[int(obj.IsSelected(False)) if obj is not None and not obj.IsDeleted else 0
                                                 for key in keys for obj in [doc.Objects.FindId(key)]]))

        def selected(indices):
            doc.Objects.UnselectAll()
            for i in indices:
                if not doc.Objects.Select(keys[i]):
                    raise ValueError('owned lifetime selection failed')

        marker = 'Owned edge lifetime ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        selected(op['analyzed'])
        before = snapshot()
        success, event_state, events = observe_command(R.Commands.Command, 'ShowEdges',
                lambda: R.RhinoApp.RunScript('_ShowEdges', True), snapshot, lambda: [], True)
        R.RhinoApp.Wait()
        opened_panel = inspect('Edge Analysis')
        labels = {'All':'All edges', 'Naked':'Naked edges', 'NonManifold':'Non-manifold edges'}
        inspect('Edge Analysis', labels[op.get('initial_mode','Naked')])
        stage('Start')
        if op['removed']:
            selected(op['removed'])
            inspect('Edge Analysis', 'Remove Objects')
            stage('Remove')
        selected(op.get('targets', op.get('deleted')))
        edit = op.get('edit', 'Delete')
        if edit == 'Select':
            stage('Select')
            for i, mode in enumerate(op.get('mode_sequence',[op['panel_mode']])):
                if i:
                    selected(op.get('targets',op.get('deleted')))
                    stage('Select'+str(i))
                if mode != 'Zoom':
                    inspect('Edge Analysis', labels[mode])
                    stage('Mode'+str(i) if op.get('mode_sequence') else 'Mode')
        else:
            macro = '_Move w0,0,0 w7,2,1' if edit == 'Move' else '_'+edit
            for action, macro in [(edit, macro), ('Undo'+edit, '_Undo'),
                                  ('Redo'+edit, '_Redo'), ('Restore', '_Undo')]:
                result = bool(R.RhinoApp.RunScript(macro, True))
                commands.append(dict(action=action, macro=macro, success=result))
                stage(action)
        controls = inspect('Edge Analysis')
        zoom = [c for c in controls if c['kind'] == 'Button' and c['label'].replace('&', '') == 'Zoom']
        available = len(zoom) == 1 and zoom[0]['enabled']
        if available and not op.get('skip_zoom'):
            finish_input = start(R, S, ['_All', '_Mark', ''])
            inspect('Edge Analysis', 'Zoom', physical=True)
            R.RhinoApp.Wait()
            zoom_input = finish_input()
        else:
            zoom_input = dict(unavailable=True, controls=controls)
        stage('Zoom')
        after = snapshot()
        if after['points']:
            commands.append(dict(action='UndoMark', macro='_Undo', success=bool(R.RhinoApp.RunScript('_Undo', True))))
            stage('UndoMark')
            commands.append(dict(action='RedoMark', macro='_Redo', success=bool(R.RhinoApp.RunScript('_Redo', True))))
            stage('RedoMark')
        return dict(before=before, stages=stages, after=after, success=success,
                    event_state=event_state, events=events, commands=commands,
                    source_archive=archive, zoom_input=zoom_input, opened_panel=opened_panel,
                    history=R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()), 0
    finally:
        if finish_input is not None:finish_input.stop()
        R.RhinoApp.RunScript('!', False)
        R.RhinoApp.RunScript('_ShowEdgesOff', False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id, True)
        doc.SubDAppearance = old
