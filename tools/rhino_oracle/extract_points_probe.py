"""Public ExtractPt on owned control/edit grips, with metadata and history."""
from copy import deepcopy
try:
    from .extract_points_cases import ALL_CASES, validate
except (ImportError, ValueError, SystemError):
    from extract_points_cases import ALL_CASES, validate


def run(operation, host, iterations, tolerance):
    validate(operation)
    from extract_points_geometry import create
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('ExtractPt needs an empty owned idle document')
    case = deepcopy(ALL_CASES[operation['case']])
    ids, layers, groups = [], [], []
    old_layer = doc.Layers.CurrentLayerIndex
    old_appearance = doc.SubDAppearance

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            a, g = obj.Attributes, obj.Geometry
            points = ([host['_xyz'](g.Location)] if isinstance(g, G.Point) else
                      [host['_xyz'](p) for p in g.GetPoints()] if isinstance(g, G.PointCloud) else None)
            rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                id=str(obj.Id), name=a.Name, layer=doc.Layers[a.LayerIndex].Name,
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                color_source=int(a.ColorSource), groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])],
                selected=bool(obj.IsSelected(False)), crc=int(g.DataCRC(0)),
                points=points, cloud=isinstance(g, G.PointCloud), grips_on=bool(obj.GripsOn),
                grips=[dict(index=int(p.Index), point=host['_xyz'](p.CurrentLocation),
                            selected=bool(p.IsSelected(False)), hidden=bool(p.IsHidden))
                       for p in (obj.GetGrips() or [])]))
            if case.get('component_capture'):
                from extract_points_component_input import selected
                rows[-1]['components'] = selected(obj,host) if obj.Id in ids else []
        return rows

    def archive():
        model = R.FileIO.File3dm()
        try:
            layer_map = {i:model.Layers.AddLayer(doc.Layers[i].Name,doc.Layers[i].Color) for i in layers}
            for key in ids:
                obj = doc.Objects.FindId(key)
                a = obj.Attributes.Duplicate()
                a.LayerIndex = layer_map[a.LayerIndex]
                a.RemoveFromAllGroups()
                model.Objects.Add(obj.Geometry, a)
            model.Settings.ModelAbsoluteTolerance = doc.ModelAbsoluteTolerance
            return S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()

    try:
        record = doc.BeginUndoRecord('Owned ExtractPt sources')
        try:
            for index, recipe in enumerate(case['sources']):
                layer = doc.Layers.Add('Extract input '+str(index), S.Drawing.Color.FromArgb(40,80,120))
                layers.append(layer)
                g = create(recipe['kind'], host)
                try:
                    g.Transform(G.Transform.Translation(20.*index, 0., 0.))
                    a = R.DocObjects.ObjectAttributes()
                    a.Name, a.LayerIndex = 'Input '+str(index), layer
                    a.ObjectColor = S.Drawing.Color.FromArgb(17+index,43+index,91+index)
                    a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                    ids.append(doc.Objects.Add(g, a))
                finally:
                    g.Dispose()
            layers.append(doc.Layers.Add('Extract current', S.Drawing.Color.FromArgb(20,40,60)))
            doc.Layers.SetCurrentLayerIndex(layers[-1], True)
            groups.append(doc.Groups.Add('Extract assembly', S.Array[S.Guid](ids)))
        finally:
            doc.EndUndoRecord(record)
        if any(key == S.Guid.Empty for key in ids) or min(layers+groups) < 0:
            raise ValueError('invalid owned ExtractPt source tables')
        source_3dm = archive()
        if case.get('component_getter'):host['_record_progress']('ExtractPt getter: source archive ready')
        for key, recipe in zip(ids, case['sources']):
            if recipe['mode'] == 'edit':
                doc.Objects.Select(key, True)
                if not R.RhinoApp.RunScript('_EditPtOn', False):
                    raise ValueError('could not enable owned edit grips')
                doc.Objects.UnselectAll()
            elif recipe['mode'] == 'control':
                doc.Objects.FindId(key).GripsOn = True
        if case.get('component_getter'):host['_record_progress']('ExtractPt getter: prepare visible grips')
        if case.get('getter') or case.get('component_getter'):
            hidden_candidates = 0
            for key, recipe in zip(ids, case['sources']):
                for grip in (doc.Objects.FindId(key).GetGrips() or []):
                    hide = int(grip.Index) not in recipe['grips']
                    grip.Select(hide)
                    hidden_candidates += int(hide)
            if hidden_candidates and not R.RhinoApp.RunScript('_HidePt', False):
                raise ValueError('could not hide unrelated getter grips')
            doc.Objects.UnselectAll()
        doc.ClearUndoRecords(True)
        doc.ClearRedoRecords()
        if case.get('component_capture'):
            from extract_points_component_input import select
            doc.SubDAppearance = G.SubDComponentLocation.ControlNet if case['flat'] else G.SubDComponentLocation.Surface
            select(doc,ids,case,host)
        for index in case['whole']:
            doc.Objects.Select(ids[index], True)
        for index in case['order']:
            grips = list(doc.Objects.FindId(ids[index]).GetGrips() or [])
            for pick in case['sources'][index]['grips']:
                if not 0 <= pick < len(grips):
                    raise ValueError('owned ExtractPt grip index outside source')
                grips[pick].Select(True)
                if not grips[pick].IsSelected(False):
                    raise ValueError('owned ExtractPt grip selection failed')
        before = snapshot()
        if case.get('component_getter'):host['_record_progress']('ExtractPt getter: before snapshot ready')
        component_records = None
        if case.get('component_records'):
            component_records = [[dict(kind=str(c.ComponentIndexType), index=int(c.Index)) for c in (doc.Objects.FindId(key).GetSelectedSubObjects() or [])] for key in ids]
        profile = None
        if case.get('curve_profile'):
            g = doc.Objects.FindId(ids[0]).Geometry
            g.UpdateSurfaceMeshCache(False)
            profiles = []
            edges = list(g.Edges) if case.get('all_edge_profiles') else [list(g.Edges)[0]]
            for edge in edges:
                curve = edge.ToNurbsCurve(True)
                if curve is None:
                    raise ValueError('native SubD edge curve unavailable')
                try:
                    profiles.append(dict(degree=int(curve.Degree), knots=[float(k) for k in curve.Knots],
                        controls=[host['_xyz'](p.Location) for p in curve.Points],
                        greville=[host['_xyz'](curve.GrevillePoint(i)) for i in range(curve.Points.Count)],
                        samples=[host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i/16.))) for i in range(17)]))
                finally:
                    curve.Dispose()
            profile = profiles if case.get('all_edge_profiles') else profiles[0]
        options = ['_OutputLayer=_'+case['layer'], '_Output=_'+case['output']]
        if case.get('output_first'):
            options.reverse()
        if case.get('component_capture') and not case.get('component_getter'):
            options = options[:1]
        macro = '_-ExtractPt '+' '.join(options)+('' if case.get('getter') or case.get('component_getter') else ' _Enter')
        history = R.RhinoApp.CommandHistoryWindowText
        receipt = []
        def execute():
            if case.get('ranges'):
                doc.Objects.UnselectAll()
                script='_-ExtractPt '+' '.join(options)+' '
                script+=' '.join('_SelID '+str(ids[i]) for i in case['whole'])
                parameters, points = [], []
                for index,start,end in case['ranges']:
                    curve=doc.Objects.FindId(ids[index]).Geometry
                    interval=[curve.Domain.ParameterAt(f) for f in [start,end]]
                    endpoints=[host['_xyz'](curve.PointAt(t)) for t in interval]
                    parameters.append([index]+interval)
                    points.append([index]+endpoints)
                    tokens=[','.join(repr(float(x)) for x in p) for p in endpoints]
                    script+=' _SubCrv _SelID '+str(ids[index])+' '+tokens[0]+' '+tokens[1]
                script+=(' _SelPt' if case.get('range_grips') else '')+' _Enter _Cancel _Cancel'
                receipt.append(dict(parameters=parameters, points=points, script=script))
                return R.RhinoApp.RunScript(script, True)
            if case.get('component_getter'):
                from extract_points_getter import run as drive_getter
                success, trace = drive_getter(doc,ids,case,host,macro,operation['id'])
                receipt.append(trace)
                return success
            if case.get('layer_target') is not None:
                # Option discovery only: rejected tokens can leave the command
                # and trailing selections outside the observed ExtractPt event.
                if case.get('target_getter'):doc.Objects.UnselectAll()
                script='_-ExtractPt '+('_OutputLayer _TargetObject' if case.get('target_split') else '_OutputLayer=_TargetObject')+' _SelID '+str(ids[case['layer_target']])+' _Enter'
                if case.get('target_getter'):
                    script+=' '+' '.join('_SelID '+str(ids[i]) for i in case['whole'])+' _Enter'
                receipt.append(dict(script=script))
                return R.RhinoApp.RunScript(script,bool(case.get('target_echo')))
            if not case.get('getter'):
                return R.RhinoApp.RunScript(macro, False)
            doc.Objects.UnselectAll()
            for key in ids:
                for grip in (doc.Objects.FindId(key).GetGrips() or []):
                    grip.Select(False)
            selection = ' '.join('_SelID '+str(ids[i]) for i in case['whole'])
            receipt.append(dict(selection=selection, visible_grips=sum(len(s['grips']) for s in case['sources'])))
            return R.RhinoApp.RunScript(macro+' '+selection+' _SelPt _Enter', False)
        if case.get('component_getter'):
            execute()
            success = receipt[-1]['command_success']
            after = snapshot()
            events = [dict(name='ExtractPt',result='Success' if success else 'Failure',
                capture='Managed EndCommand receipt; document snapshot after the bounded selection macro')]
        else:
            success, after, events = observe_command(R.Commands.Command, 'ExtractPt',
                execute, snapshot, lambda: [], True)
        text = R.RhinoApp.CommandHistoryWindowText
        history = text[len(history):] if text.startswith(history) else text[-3000:]
        if not success:
            raise ValueError('owned ExtractPt did not succeed: '+str(receipt)+'; history: '+history[-1500:])
        if not R.RhinoApp.RunScript('_Undo', False):
            raise ValueError('owned ExtractPt Undo failed')
        undo = snapshot()
        if not R.RhinoApp.RunScript('_Redo', False):
            raise ValueError('owned ExtractPt Redo failed')
        redo = snapshot()
        return dict(input=case, source_3dm=source_3dm, before=before, after=after,
                    undo=undo, redo=redo, success=success, events=events, macro=macro,
                    history=history, getter=receipt[0] if receipt else None, curve_profile=profile, component_records=component_records), 0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:
                obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
        for group in reversed(groups):
            doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old_layer, True)
        doc.SubDAppearance = old_appearance
        for layer in reversed(layers):
            doc.Layers.Delete(layer, True)
