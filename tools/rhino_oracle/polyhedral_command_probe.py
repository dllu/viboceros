"""Closed interactive Boolean recipes on earlier results and compound solids."""
import re

SHAPES = ('concave', 'hole', 'cavity', 'island', 'disjoint_shells', 'two_holes',
          'rounded_uv', 'reversed_hole', 'coplanar_concave', 'singular_two_holes')
SPECS = {prefix+'_'+shape: dict(command=command, shape=shape)
         for prefix, command in (('u', 'BooleanUnion'), ('i', 'BooleanIntersection'),
                                 ('d', 'BooleanDifference')) for shape in SHAPES}
for name, values in (
    ('u_hole_pre', dict(pre=True, history=True)),
    ('u_hole_keep', dict(delete=False, history=True)),
    ('u_hole_nomerge', dict(merge=False)),
    ('u_concave_reverse', dict(shape='concave', reverse=True)),
    ('u_disjoint_member', dict(shape='concave', extra='disjoint')),
    ('u_partial_multishell', dict(shape='partial_multishell')),
    ('u_partial_multishell_keep', dict(shape='partial_multishell', delete=False)),
    ('u_partial_multishell_pre', dict(shape='partial_multishell', pre=True)),
    ('u_hole_three', dict(extra='union')),
    ('i_hole_common', dict(common=True)),
    ('i_hole_pre', dict(pre=True, history=True)),
    ('i_hole_keep', dict(delete=False)),
    ('i_hole_three_common', dict(common=True, extra='common')),
    ('i_hole_three_sets', dict(extra='first_set')),
    ('i_disjoint_shells_keep', dict(shape='disjoint_shells', delete=False)),
    ('i_disjoint_shells_pre', dict(shape='disjoint_shells', pre=True)),
    ('d_hole_pre', dict(pre=True, history=True)),
    ('d_hole_keep', dict(delete=False)),
    ('d_hole_keep_cutters', dict(cutters=False, history=True)),
    ('d_hole_split', dict(split=True)),
    ('d_hole_split_pre', dict(split=True, pre=True, history=True)),
    ('d_hole_three_cutters', dict(extra='cutter')),
    ('d_two_targets', dict(extra='target')),
    ('d_noninteracting_target', dict(extra='disjoint_target')),
    ('d_disjoint_shells_keep', dict(shape='disjoint_shells', delete=False)),
    ('u_two_components', dict(shape='concave', extra='two_pairs')),
    ('i_two_components', dict(shape='concave', extra='two_pairs')),
    ('u_cavity_unopened', dict(shape='cavity', unopened=True)),
    ('i_cavity_unopened', dict(shape='cavity', unopened=True)),
    ('d_cavity_unopened', dict(shape='cavity', unopened=True)),
    ('i_disjoint_shells_common', dict(shape='disjoint_shells', common=True)),
    ('d_two_targets_split', dict(split=True, extra='target')),
    ('i_cavity_first_multi', dict(shape='cavity', extra='first_set')),
    ('i_cavity_second_multi', dict(shape='cavity', extra='second_set')),
    ('d_partial_multishell', dict(shape='partial_multishell')),
):
    SPECS[name] = dict(dict(command={'u': 'BooleanUnion', 'i': 'BooleanIntersection',
                                   'd': 'BooleanDifference'}[name[0]], shape='hole'), **values)
CASES = tuple(SPECS)


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 96):
        raise ValueError('polyhedral commands require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'polyhedral_boolean_command' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid polyhedral command recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='polyhedral_boolean_command', id='polycmd_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('polyhedral commands require idle execution')
    doc, G = Rhino.RhinoDoc.ActiveDoc, Rhino.Geometry
    if list(doc.Objects):
        raise ValueError('polyhedral commands require an empty owned document')
    from join_probe import observe_command
    xyz = host['_xyz']
    ids, layers, groups, owned = [], [], [], []
    saved_layer, saved_tolerance = doc.Layers.CurrentLayerIndex, doc.ModelAbsoluteTolerance
    doc.ModelAbsoluteTolerance = 1e-7
    spec = SPECS[op['case']]
    command = spec['command']
    delete, merge, cutters = spec.get('delete', True), spec.get('merge', True), spec.get('cutters', True)

    def keep(g):
        if g is None:
            raise ValueError('polyhedral command source construction failed')
        owned.append(g)
        return g

    def box(bounds):
        return keep(G.BoundingBox(G.Point3d(*[p[0] for p in bounds]), G.Point3d(*[p[1] for p in bounds])).ToBrep())

    def one(a, b, operation):
        result = (G.Brep.CreateBooleanUnion([a, b], 1e-7) if operation == 'union'
                  else G.Brep.CreateBooleanIntersection(a, b, 1e-7) if operation == 'intersection'
                  else G.Brep.CreateBooleanDifference(a, b, 1e-7))
        values = [] if result is None else [keep(g) for g in result]
        if len(values) != 1:
            raise ValueError('polyhedral source must yield one body')
        return values[0]

    def clear_objects():
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            g, a = obj.Geometry, obj.Attributes
            if not isinstance(g, G.Brep):
                raise ValueError('unexpected polyhedral command document geometry')
            area = G.AreaMassProperties.Compute(g)
            mass = G.VolumeMassProperties.Compute(g) if g.IsSolid else None
            try:
                row = dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                           selected=bool(obj.IsSelected(False)), kind='Brep', name=a.Name,
                           layer=layers.index(a.LayerIndex) if a.LayerIndex in layers else None,
                           color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                           groups=sorted(groups.index(i) for i in (a.GetGroupList() or []) if i in groups),
                           attribute_text=a.GetUserString('Code'), geometry_text=g.GetUserString('Code'),
                           valid=bool(g.IsValid), solid=bool(g.IsSolid), orientation=str(g.SolidOrientation),
                           faces=int(g.Faces.Count), edges=int(g.Edges.Count),
                           volume=float(mass.Volume) if mass else None, area=float(area.Area) if area else None,
                           centroid=xyz(mass.Centroid) if mass else None,
                           vertices=[xyz(v.Location) for v in g.Vertices],
                           face_regions=[[[xyz(f.PointAt(t.PointAtStart.X, t.PointAtStart.Y))
                                           for t in loop.Trims] for loop in f.Loops] for f in g.Faces])
                rows.append(row)
            finally:
                if area: area.Dispose()
                if mass: mass.Dispose()
        return rows

    def invoke(macro, name):
        marker = 'Viboceros polyhedral command '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success, after, events = observe_command(Rhino.Commands.Command, name,
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        if Rhino.Commands.Command.InCommand():
            raise ValueError('polyhedral command remains active')
        return dict(success=success, macro=macro, after=after, after_script=snapshot(), events=events,
                    history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1])

    def selectors(indices):
        return ' '.join('_SelID '+str(ids[i]) for i in indices)

    def options():
        text = '_DeleteInput=_'+('Yes' if delete else 'No')
        if command == 'BooleanUnion': text += ' _MergeCoplanarFaces=_'+('Yes' if merge else 'No')
        if command == 'BooleanDifference' and delete: text += ' _DeleteCutters=_'+('Yes' if cutters else 'No')
        return text

    try:
        # Seed options before any preselection can cause immediate acceptance.
        for bounds in (((100., 102.),)*3, ((101., 103.),)*3):
            ids.append(doc.Objects.AddBrep(box(bounds)))
        seed = '_-'+command+' '+options()+' '+selectors([0])
        seed += (' '+selectors([1])+' _Enter' if command == 'BooleanUnion'
                 else ' _Enter '+selectors([1])+' _Enter')
        if not Rhino.RhinoApp.RunScript(seed, False):
            raise ValueError('polyhedral command preference seed failed')
        clear_objects(); ids[:] = []
        shape = spec['shape']
        a, b = box(((0., 3.),)*3), box(((1.5, 3.5), (1.5, 3.5), (1., 2.)))
        if shape in ('concave', 'coplanar_concave'):
            a = one(a, box(((2., 4.), (1., 2.), (0., 3.))), 'union')
            if shape == 'coplanar_concave': b = box(((2., 4.), (1., 3.), (0., 3.)))
        elif shape in ('hole', 'reversed_hole', 'two_holes', 'singular_two_holes'):
            a = one(a, box(((1., 2.), (1., 2.), (-1., 4.))), 'difference')
            if shape == 'reversed_hole': a.Flip()
            if shape in ('two_holes', 'singular_two_holes'):
                interval = (2.25, 3.25) if shape == 'two_holes' else (2., 3.)
                b = one(box(((1., 4.),)*3), box((interval, interval, (0., 5.))), 'difference')
        elif shape == 'cavity':
            inner = box(((1., 2.),)*3); inner.Flip(); a.Append(inner)
        elif shape == 'island':
            a = box(((0., 4.),)*3)
            inner = box(((1., 3.),)*3); inner.Flip(); a.Append(inner)
            a.Append(box(((1.5, 2.5),)*3)); b = box(((2., 5.),)*3)
        elif shape in ('disjoint_shells', 'partial_multishell'):
            a.Append(box(((4., 5.),)*3 if shape == 'disjoint_shells' else ((10., 11.),)*3))
            if shape == 'disjoint_shells': b = box(((2., 4.5),)*3)
        elif shape == 'rounded_uv':
            a = one(a, box(((.5, 2.5), (-1., 4.), (-1., 4.))), 'intersection')
        else: raise ValueError('unknown owned shape')
        if spec.get('split'): b = box(((1.5, 2.5), (-1., 4.), (-1., 4.)))
        if spec.get('unopened'): b = box(((2.5, 3.5), (2.5, 3.5), (1., 2.)))
        geometries = [a, b]
        first, second = [0], [1]
        extra = spec.get('extra')
        if extra in ('disjoint', 'disjoint_target'): geometries.append(box(((10., 11.),)*3))
        elif extra in ('union', 'cutter'): geometries.append(box(((1.5, 2.5), (.5, 1.5), (1., 2.))))
        elif extra == 'common': geometries.append(box(((1.75, 4.), (1.5, 3.5), (.5, 2.5))))
        elif extra in ('first_set', 'second_set'): geometries.append(box(((.75, 2.25),)*3))
        elif extra == 'target': geometries.append(box(((1., 4.),)*3))
        elif extra == 'two_pairs':
            geometries.extend([box(((10., 12.),)*3), box(((11., 13.),)*3)])
        if extra in ('target', 'disjoint_target', 'first_set'): first.append(2)
        elif extra in ('cutter', 'second_set'): second.append(2)
        elif extra == 'two_pairs': first.append(2); second.append(3)
        if any(not g.IsValid or not g.IsSolid for g in geometries):
            raise ValueError('polyhedral command sources must be valid solids')
        serial = doc.BeginUndoRecord('Polyhedral command owned sources')
        if not serial: raise ValueError('source undo record failed')
        try:
            for i, g in enumerate(geometries):
                layer = Rhino.DocObjects.Layer()
                attrs = Rhino.DocObjects.ObjectAttributes()
                try:
                    layer.Name = 'Viboceros polyhedral '+str(System.Guid.NewGuid())
                    index = doc.Layers.Add(layer)
                    if index < 0: raise ValueError('source layer failed')
                    layers.append(index)
                    attrs.LayerIndex, attrs.Name = index, 'source-'+str(i)
                    attrs.ObjectColor = System.Drawing.Color.FromArgb(20+i, 40, 60)
                    attrs.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                    attrs.SetUserString('Code', 'attribute-'+str(i)); g.SetUserString('Code', 'geometry-'+str(i))
                    key = doc.Objects.AddBrep(g, attrs)
                    if key == System.Guid.Empty: raise ValueError('source insertion failed')
                    ids.append(key)
                finally: attrs.Dispose(); layer.Dispose()
            for members in [[key] for key in ids]+[ids]:
                index = doc.Groups.Add('Viboceros polyhedral '+str(System.Guid.NewGuid()), members)
                if index < 0: raise ValueError('source group failed')
                groups.append(index)
        finally: doc.EndUndoRecord(serial)
        before = snapshot()
        if command == 'BooleanUnion':
            order = list(range(len(ids)))
            if spec.get('reverse'): order.reverse()
            if spec.get('pre'):
                for i in order: doc.Objects.Select(ids[i])
                macro = '_-'+command+' _Enter'
            else: macro = '_-'+command+' '+options()+' '+selectors(order)+' _Enter'
        elif spec.get('common'):
            macro = '_-'+command+' '+options()+' '+selectors(list(range(len(ids))))+' _Enter _Enter'
        else:
            if spec.get('pre'):
                for i in first: doc.Objects.Select(ids[i])
                macro = '_-'+command+' '+selectors(second)+' _Enter'
            else: macro = '_-'+command+' '+options()+' '+selectors(first)+' _Enter '+selectors(second)+' _Enter'
        result = dict(before=before, first=first, second=second, command=invoke(macro, command))
        if spec.get('history'):
            result['undo'] = invoke('_Undo', 'Undo'); result['redo'] = invoke('_Redo', 'Redo')
        return result, 0
    finally:
        clear_objects()
        for index in groups: doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer, True)
        for index in reversed(layers): doc.Layers.Delete(index, True)
        doc.ModelAbsoluteTolerance = saved_tolerance
        for g in reversed(owned): g.Dispose()
