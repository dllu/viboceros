"""Actual typed selection commands over owned extrusion and instance scenes."""
COMMANDS = ['SelExtrusion', 'SelBlockInstance', 'SelMirroredBlocks']
SCENARIOS = ['base', 'preselected', 'grouped', 'hidden', 'locked',
             'hidden-layer', 'locked-layer']


def request():
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10),
                operations=[dict(op='typed_selection', id=command+'-'+scenario,
                                 command=command, scenario=scenario)
                            for command in COMMANDS for scenario in SCENARIOS])


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'command', 'scenario'}
            or op['op'] != 'typed_selection' or op['command'] not in COMMANDS
            or op['scenario'] not in SCENARIOS+['no-matches','no-matches-preselected']
            or op['id'] != op['command']+'-'+op['scenario']):
        raise ValueError('fixed typed selection recipe required')


def run(op, host, iterations, tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    prefix = 'TypedSelection-'+op['id']+'-'
    ids, definitions, groups, layers, owned = [], [], [], [], []
    def own(geometry):
        if geometry is None: raise ValueError('typed scene construction failed')
        owned.append(geometry)
        return geometry
    def matrix(x=1., y=1., z=1., shear=0.):
        m = G.Transform.Identity
        m.M00, m.M11, m.M22, m.M01 = x, y, z, shear
        return m
    def attributes(): return R.DocObjects.ObjectAttributes()
    def definition(name, geometries):
        index = doc.InstanceDefinitions.Add(prefix+name, '', G.Point3d.Origin,
            S.Array[G.GeometryBase](geometries),
            S.Array[R.DocObjects.ObjectAttributes]([attributes() for g in geometries]))
        if index < 0: raise ValueError('typed block definition failed')
        definitions.append(index)
        return index
    def snapshot():
        rows = []
        for key in ids:
            obj = doc.Objects.FindId(key)
            if obj is None: raise ValueError('selection lost source')
            a = obj.Attributes
            rows.append(dict(crc=int(obj.Geometry.DataCRC(0)), layer=int(a.LayerIndex),
                             mode=str(a.Mode), groups=list(a.GetGroupList() or [])))
        return dict(objects=rows, definitions=[[int(o.Geometry.DataCRC(0)) for o in doc.InstanceDefinitions[i].GetObjects()] for i in definitions],
                    layers=[(bool(doc.Layers[i].IsVisible),bool(doc.Layers[i].IsLocked)) for i in layers])
    try:
        profile = own(G.PolylineCurve(S.Array[G.Point3d]([
            G.Point3d(0,0,0),G.Point3d(2,0,0),G.Point3d(2,3,0),
            G.Point3d(0,3,0),G.Point3d(0,0,0)])))
        extrusion = own(G.Extrusion.Create(profile, 4., True))
        brep = own(extrusion.ToBrep())
        surface = own(G.NurbsSurface.CreateFromCorners(
            G.Point3d(0,0,0),G.Point3d(2,0,0),G.Point3d(2,3,0),G.Point3d(0,3,0)))
        line = own(G.LineCurve(G.Point3d(0,0,0),G.Point3d(2,0,0)))
        point = own(G.Point(G.Point3d(13,0,0)))
        for geometry in [extrusion, brep, surface, line, point]:
            key = doc.Objects.Add(geometry, attributes())
            if key == S.Guid.Empty: raise ValueError('typed source insertion failed')
            ids.append(key)
        leaf = definition('Leaf', [line, extrusion])
        inner = own(G.InstanceReferenceGeometry(doc.InstanceDefinitions[leaf].Id, matrix(-1.)))
        nested = definition('Nested', [inner])
        references = [(leaf,matrix()),(leaf,matrix(-1.)),(leaf,matrix(-1.,-1.)),
                      (leaf,matrix(-1.,-1.,-1.)),(nested,matrix()),
                      (nested,matrix(-1.)),(leaf,matrix(1.,-1.,1.,2.)),(leaf,matrix())]
        for index, placement in references:
            key = doc.Objects.AddInstanceObject(index, placement, attributes())
            if key == S.Guid.Empty: raise ValueError('typed instance insertion failed')
            ids.append(key)
        scenario = op['scenario']
        if scenario == 'grouped':
            for i, members in enumerate([[0,3],[5,4]]):
                groups.append(doc.Groups.Add(prefix+str(i), S.Array[S.Guid]([ids[j] for j in members])))
        if scenario in ['hidden-layer','locked-layer']:
            layer = R.DocObjects.Layer()
            try:
                layer.Name = prefix+'Restricted'
                layer.IsVisible = scenario != 'hidden-layer'
                layer.IsLocked = scenario == 'locked-layer'
                index = doc.Layers.Add(layer)
            finally: layer.Dispose()
            if index < 0: raise ValueError('typed layer insertion failed')
            layers.append(index)
            for i in [0,5,6]:
                a = doc.Objects.FindId(ids[i]).Attributes.Duplicate()
                try:
                    a.LayerIndex = index
                    if not doc.Objects.ModifyAttributes(ids[i], a, True): raise ValueError('typed layer assignment failed')
                finally: a.Dispose()
        if scenario in ['hidden','locked']:
            for i in [0,5,6]:
                if scenario == 'hidden': doc.Objects.Hide(ids[i], True)
                else: doc.Objects.Lock(ids[i], True)
        if scenario in ['no-matches','no-matches-preselected']:
            for i in [0]+list(range(5,13)): doc.Objects.Hide(ids[i], True)
        # A real command creates a redo branch. The selector must retain it.
        if not R.RhinoApp.RunScript('_Point w99,99,99', False): raise ValueError('redo sentinel creation failed')
        R.RhinoApp.RunScript('_Undo', False)
        doc.Objects.UnselectAll()
        if scenario in ['preselected','no-matches-preselected']: doc.Objects.Select(ids[4])
        before = snapshot()
        events = []
        def ended(sender, event):
            if event.CommandEnglishName == op['command']: events.append(str(event.CommandResult))
        R.Commands.Command.EndCommand += ended
        try: R.RhinoApp.RunScript('_'+op['command'], False)
        finally: R.Commands.Command.EndCommand -= ended
        if len(events) != 1 or events[0] not in ['Success','Nothing']:
            raise ValueError('typed selection command did not complete: '+str(events))
        selected = [i for i,key in enumerate(ids) if doc.Objects.FindId(key).IsSelected(False)]
        unchanged = before == snapshot()
        types = [str(doc.Objects.FindId(key).Geometry.GetType().Name) for key in ids]
        R.RhinoApp.RunScript('_Redo', False)
        redo = any(isinstance(obj.Geometry,G.Point) and obj.Geometry.Location.DistanceTo(G.Point3d(99,99,99)) < 1e-12 for obj in doc.Objects)
        if not unchanged: raise ValueError('typed selection changed its sources')
        return dict(selected=selected, types=types, unchanged=unchanged, redo_preserved=bool(redo)), 0
    finally:
        # Restore only this owned scene's restrictions before deleting it.
        # Native deletion refuses locked objects and objects on locked layers.
        for index in layers:
            layer = doc.Layers[index]
            try:
                layer.IsVisible, layer.IsLocked = True, False
                layer.CommitChanges()
            finally: layer.Dispose()
        for obj in list(doc.Objects):
            if obj.IsLocked: doc.Objects.Unlock(obj.Id, True)
            if obj.IsHidden: doc.Objects.Show(obj.Id, True)
            doc.Objects.Delete(obj.Id, True)
        for index in reversed(definitions):
            if not doc.InstanceDefinitions[index].IsDeleted: doc.InstanceDefinitions.Delete(index, True, True)
        for index in groups: doc.Groups.Delete(index)
        for index in reversed(layers): doc.Layers.Delete(index, True)
        for geometry in reversed(owned): geometry.Dispose()
