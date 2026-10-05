"""Closed public BooleanUnion command, metadata, and selection recipes."""
import re

CASES = ('defaults', 'corner', 'delete_no', 'merge_yes', 'post_reverse', 'pre_reverse',
         'partial_coplanar', 'partial_merge', 'touch', 'touch_merge', 'disjoint',
         'contained', 'equal', 'three_chain', 'three_shared', 'three_disjoint',
         'mixed', 'single', 'open_surface', 'cancel_selection', 'cancel_options',
         'undo_redo', 'remember', 'boundary_contained', 'three_contained',
         'touch_edge', 'touch_point', 'three_shared_merge', 'pre_delete_no',
         'cancel_picked', 'coplanar_equal', 'face_priority')


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('BooleanUnion commands require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'boolean_union_command' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid BooleanUnion command recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='boolean_union_command', id='boolean_union_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    G, doc = Rhino.Geometry, Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('BooleanUnion commands require idle execution')
    if list(doc.Objects):
        raise ValueError('BooleanUnion commands require an empty owned document')
    from join_probe import observe_command
    ids, layers, groups = [], [], []
    saved_layer = doc.Layers.CurrentLayerIndex
    saved_tolerance = doc.ModelAbsoluteTolerance
    doc.ModelAbsoluteTolerance = 1e-7
    xyz = host['_xyz']
    case = op['case']

    def box(bounds):
        return G.BoundingBox(G.Point3d(*[p[0] for p in bounds]), G.Point3d(*[p[1] for p in bounds])).ToBrep()

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            g, a = obj.Geometry, obj.Attributes
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                       selected=bool(obj.IsSelected(False)), kind=g.GetType().Name,
                       name=a.Name, layer=layers.index(a.LayerIndex) if a.LayerIndex in layers else None,
                       color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                       groups=sorted(groups.index(i) for i in (a.GetGroupList() or []) if i in groups),
                       attribute_text=a.GetUserString('Code'), geometry_text=g.GetUserString('Code'))
            if isinstance(g, G.Brep):
                area = G.AreaMassProperties.Compute(g)
                mass = G.VolumeMassProperties.Compute(g) if g.IsSolid else None
                try:
                    row.update(valid=bool(g.IsValid), solid=bool(g.IsSolid), faces=int(g.Faces.Count),
                               edges=int(g.Edges.Count), volume=float(mass.Volume) if mass else None,
                               area=float(area.Area) if area else None,
                               centroid=xyz(mass.Centroid) if mass else None,
                               vertices=[xyz(v.Location) for v in g.Vertices],
                               face_regions=[[[xyz(f.PointAt(t.PointAtStart.X,t.PointAtStart.Y))
                                               for t in loop.Trims] for loop in f.Loops] for f in g.Faces])
                finally:
                    if area: area.Dispose()
                    if mass: mass.Dispose()
            elif isinstance(g, G.Point):
                row['point'] = xyz(g.Location)
            else:
                raise ValueError('unexpected BooleanUnion document geometry')
            rows.append(row)
        return rows

    def invoke(macro, command='BooleanUnion'):
        marker = 'Viboceros BooleanUnion '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success, after, events = observe_command(Rhino.Commands.Command, command,
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        if Rhino.Commands.Command.InCommand():
            raise ValueError('BooleanUnion command remains active')
        return dict(success=success, macro=macro, after=after, after_script=snapshot(), events=events,
                    history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1])

    def clear_objects():
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)

    def add_sources():
        geometries = [box(((0., 4.), (0., 1.), (0., 1.))) if case=='face_priority' else box(((0., 2.),)*3)]
        if case == 'disjoint': bounds = ((4., 5.),)*3
        elif case == 'contained': bounds = ((.5, 1.5),)*3
        elif case == 'equal': bounds = ((0., 2.),)*3
        elif case == 'boundary_contained': bounds = ((0., 1.),)*3
        elif case == 'touch_edge': bounds = ((2., 4.), (2., 4.), (0., 2.))
        elif case == 'touch_point': bounds = ((2., 4.),)*3
        elif case == 'face_priority': bounds = ((1., 3.), (0., 2.), (0., 1.))
        elif case.startswith('touch'): bounds = ((2., 4.), (0., 2.), (0., 2.))
        elif case.startswith('partial') or case == 'defaults': bounds = ((1., 3.), (0., 2.), (.5, 1.5))
        elif case in ('three_shared','three_shared_merge','coplanar_equal'): bounds = ((1., 3.), (0., 2.), (0., 2.))
        else: bounds = ((1., 3.),)*3
        if case != 'single': geometries.append(box(bounds))
        if case == 'three_chain': geometries.append(box(((2., 4.),)*3))
        elif case in ('three_shared','three_shared_merge'): geometries.append(box(((2., 4.), (0., 2.), (0., 2.))))
        elif case == 'three_disjoint': geometries.append(box(((10., 11.),)*3))
        elif case == 'three_contained': geometries.append(box(((.5, 1.5),)*3))
        elif case == 'mixed': geometries.append(G.Point(G.Point3d(20., 1., 0.)))
        elif case == 'open_surface':
            geometries.append(G.PlaneSurface(G.Plane.WorldXY, G.Interval(-1., 4.), G.Interval(-1., 4.)).ToBrep())
        serial = doc.BeginUndoRecord('BooleanUnion owned source setup')
        if not serial: raise ValueError('BooleanUnion setup undo record failed')
        try:
            for i, g in enumerate(geometries):
                a = Rhino.DocObjects.ObjectAttributes()
                try:
                    if i >= len(layers):
                        layer = Rhino.DocObjects.Layer()
                        try:
                            layer.Name = 'Viboceros BooleanUnion '+str(System.Guid.NewGuid())
                            index = doc.Layers.Add(layer)
                            if index < 0: raise ValueError('BooleanUnion layer creation failed')
                            layers.append(index)
                        finally: layer.Dispose()
                    a.LayerIndex, a.Name = layers[i], 'source-'+str(i)
                    a.ObjectColor = System.Drawing.Color.FromArgb(20+i, 40, 60)
                    a.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                    a.SetUserString('Code', 'attribute-'+str(i))
                    g.SetUserString('Code', 'geometry-'+str(i))
                    key = doc.Objects.Add(g, a)
                    if key == System.Guid.Empty: raise ValueError('BooleanUnion source insertion failed')
                    ids.append(key)
                finally:
                    a.Dispose(); g.Dispose()
            for members in [[key] for key in ids] + [ids]:
                index = doc.Groups.Add('Viboceros BooleanUnion '+str(System.Guid.NewGuid()), members)
                if index < 0: raise ValueError('BooleanUnion group creation failed')
                groups.append(index)
        finally: doc.EndUndoRecord(serial)

    try:
        # Defaults are measured first in a fresh private settings scheme.
        # Other recipes seed deterministic options with separate owned geometry.
        if case != 'defaults':
            for bounds in (((100., 102.),)*3, ((101., 103.),)*3):
                g = box(bounds)
                try: ids.append(doc.Objects.AddBrep(g))
                finally: g.Dispose()
            macro = '_-BooleanUnion _DeleteInput=_'+('No' if case=='pre_delete_no' else 'Yes')+' _MergeCoplanarFaces=_No '+\
                    ' '.join('_SelID '+str(key) for key in ids)+' _Enter'
            if not Rhino.RhinoApp.RunScript(macro, False): raise ValueError('BooleanUnion preference seed failed')
            clear_objects(); ids[:] = []
        add_sources()
        before = snapshot()
        order = list(range(len(ids)))
        if case in ('pre_reverse', 'post_reverse'): order.reverse()
        if case in ('pre_reverse','pre_delete_no'):
            for i in order: doc.Objects.Select(ids[i])
            macro = '_-BooleanUnion _Enter'
        else:
            delete = 'No' if case in ('delete_no', 'remember') else 'Yes'
            merge = 'Yes' if case in ('merge_yes', 'partial_merge', 'touch_merge', 'remember','three_shared_merge') else 'No'
            options = '' if case == 'defaults' else '_DeleteInput=_'+delete+' _MergeCoplanarFaces=_'+merge
            selectors = ' '.join('_SelID '+str(ids[i]) for i in order)
            if case == 'cancel_selection': macro = '_-BooleanUnion _Cancel'
            elif case == 'cancel_picked': macro = '_-BooleanUnion '+options+' '+selectors+' _Cancel'
            elif case == 'cancel_options': macro = '_-BooleanUnion _DeleteInput=_No _MergeCoplanarFaces=_Yes _Cancel'
            else: macro = '_-BooleanUnion '+options+' '+selectors+' _Enter'
        result = dict(before=before, command=invoke(macro))
        if case == 'undo_redo':
            result['undo'] = invoke('_Undo', 'Undo')
            result['redo'] = invoke('_Redo', 'Redo')
        if case in ('remember', 'cancel_options'):
            clear_objects(); ids[:] = []
            for index in groups: doc.Groups.Delete(index)
            groups[:] = []
            add_sources()
            result['followup_before'] = snapshot()
            result['followup'] = invoke('_-BooleanUnion '+' '.join('_SelID '+str(key) for key in ids)+' _Enter')
        return result, 0
    finally:
        clear_objects()
        for index in groups: doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer, True)
        for index in reversed(layers): doc.Layers.Delete(index, True)
        doc.ModelAbsoluteTolerance = saved_tolerance
