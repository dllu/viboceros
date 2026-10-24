"""Public ExtractPt on owned control/edit grips, with metadata and history."""
from copy import deepcopy
try:
    from .extract_points_cases import ALL_CASES, validate
except (ImportError, ValueError):
    from extract_points_cases import ALL_CASES, validate


def run(operation, host, iterations, tolerance):
    validate(operation)
    from grip_navigation_probe import create_source
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('ExtractPt needs an empty owned idle document')
    case = deepcopy(ALL_CASES[operation['case']])
    ids, layers, groups = [], [], []
    old_layer = doc.Layers.CurrentLayerIndex

    def geometry(kind):
        if kind != 'subd':
            return create_source(kind, host)
        mesh = G.Mesh()
        try:
            for p in [[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],
                      [0.,0.,2.],[4.,0.,2.],[4.,3.,2.],[0.,3.,2.]]:
                mesh.Vertices.Add(G.Point3d(*p))
            for f in [[0,3,2,1],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7],[4,5,6,7]]:
                mesh.Faces.AddFace(*f)
            result = G.SubD.CreateFromMesh(mesh)
            if result is None:
                raise ValueError('cannot construct ExtractPt SubD source')
            return result
        finally:
            mesh.Dispose()

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
                g = geometry(recipe['kind'])
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
        for key, recipe in zip(ids, case['sources']):
            if recipe['mode'] == 'edit':
                doc.Objects.Select(key, True)
                if not R.RhinoApp.RunScript('_EditPtOn', False):
                    raise ValueError('could not enable owned edit grips')
                doc.Objects.UnselectAll()
            else:
                doc.Objects.FindId(key).GripsOn = True
        if case.get('getter'):
            for key, recipe in zip(ids, case['sources']):
                for grip in (doc.Objects.FindId(key).GetGrips() or []):
                    grip.Select(int(grip.Index) not in recipe['grips'])
            if not R.RhinoApp.RunScript('_HidePt', False):
                raise ValueError('could not hide unrelated getter grips')
            doc.Objects.UnselectAll()
        doc.ClearUndoRecords(True)
        doc.ClearRedoRecords()
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
        options = ['_OutputLayer=_'+case['layer'], '_Output=_'+case['output']]
        if case.get('output_first'):
            options.reverse()
        macro = '_-ExtractPt '+' '.join(options)+('' if case.get('getter') else ' _Enter')
        history = R.RhinoApp.CommandHistoryWindowText
        receipt = []
        def execute():
            if not case.get('getter'):
                return R.RhinoApp.RunScript(macro, False)
            doc.Objects.UnselectAll()
            for key in ids:
                for grip in (doc.Objects.FindId(key).GetGrips() or []):
                    grip.Select(False)
            selection = ' '.join('_SelID '+str(ids[i]) for i in case['whole'])
            receipt.append(dict(selection=selection, visible_grips=sum(len(s['grips']) for s in case['sources'])))
            return R.RhinoApp.RunScript(macro+' '+selection+' _SelPt _Enter', False)
        success, after, events = observe_command(R.Commands.Command, 'ExtractPt',
            execute, snapshot, lambda: [], True)
        text = R.RhinoApp.CommandHistoryWindowText
        history = text[len(history):] if text.startswith(history) else text[-3000:]
        if not success:
            raise ValueError('owned ExtractPt did not succeed')
        if not R.RhinoApp.RunScript('_Undo', False):
            raise ValueError('owned ExtractPt Undo failed')
        undo = snapshot()
        if not R.RhinoApp.RunScript('_Redo', False):
            raise ValueError('owned ExtractPt Redo failed')
        redo = snapshot()
        return dict(input=case, source_3dm=source_3dm, before=before, after=after,
                    undo=undo, redo=redo, success=success, events=events, macro=macro,
                    history=history, getter=receipt[0] if receipt else None), 0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:
                obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
        for group in reversed(groups):
            doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old_layer, True)
        for layer in reversed(layers):
            doc.Layers.Delete(layer, True)
