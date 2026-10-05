"""Closed public SDK/command witnesses for oriented compound intersections."""
import re

# Each geometry is an explicit list of (box bounds, inward) shells.
CUBE = lambda low, high: ((float(low), float(high)),)*3
SHAPES = dict(
    cavity=[(CUBE(0, 3), False), (CUBE(1, 2), True)],
    island=[(CUBE(0, 4), False), (CUBE(1, 3), True), (CUBE(1.5, 2.5), False)],
    other_cavity=[(CUBE(1.5, 3.5), False), (CUBE(2.125, 2.625), True)],
    inner=[(CUBE(1, 2), True)],
    inner_other=[(CUBE(1.5, 2.5), True)],
    cross=[(((1.5, 3.5), (1.5, 3.5), (1., 2.)), False)],
    unopened=[(((2.5, 3.5), (2.5, 3.5), (1., 2.)), False)],
    contains_inner=[(CUBE(.75, 2.25), False)],
    inside_inner=[(CUBE(1.25, 1.75), False)],
    equal_inner=[(CUBE(1, 2), False)],
    enclosing=[(CUBE(-1, 4), False)],
    equal_outer=[(CUBE(0, 3), False)],
    disjoint=[(CUBE(10, 11), False)],
    boundary_inner=[(CUBE(1.25, 2), False)],
    touches_inner=[(((2., 2.5), (1., 2.), (1., 2.)), False)],
    extra_cross=[(((2.5, 4.), (1.75, 2.75), (1., 2.)), False)],
    extra_disjoint_cross=[(((0.5, 1.5), (0.5, 1.5), (1., 2.)), False)],
)
SPECS = {}
for shape in ('cross', 'unopened', 'contains_inner', 'inside_inner', 'equal_inner',
              'enclosing', 'boundary_inner', 'touches_inner'):
    SPECS['sdk_inner_'+shape] = dict(geometries=['inner', shape], sdk=True)
    SPECS['pair_'+shape] = dict(geometries=['cavity', shape], first=[0], second=[1])
SPECS['sdk_inner_other'] = dict(geometries=['inner', 'inner_other'], sdk=True)
SPECS['pair_equal_outer'] = dict(geometries=['cavity', 'equal_outer'], first=[0], second=[1])
SPECS['pair_two_cavities'] = dict(geometries=['cavity', 'other_cavity'], first=[0], second=[1])
SPECS['pair_two_cavities_reverse'] = dict(geometries=['cavity', 'other_cavity'], first=[1], second=[0])
for side in ('first', 'second'):
    for extra in ('contains_inner', 'inside_inner', 'disjoint', 'extra_cross', 'extra_disjoint_cross'):
        first, second = ([0, 2], [1]) if side == 'first' else ([0], [1, 2])
        SPECS[side+'_'+extra] = dict(geometries=['cavity', 'cross', extra], first=first, second=second)
for side in ('first', 'second'):
    source = SPECS[side+'_contains_inner']
    for suffix, values in (('reverse', dict(reverse=True)), ('pre', dict(pre=True, history=True)),
                           ('keep', dict(delete=False, history=True))):
        spec = dict(source, **values)
        if suffix == 'reverse':
            spec[side] = list(reversed(spec[side]))
        SPECS[side+'_contains_inner_'+suffix] = spec
for extra in ('contains_inner', 'inside_inner', 'extra_cross', 'extra_disjoint_cross'):
    SPECS['common_'+extra] = dict(geometries=['cavity', 'cross', extra], first=[0, 1, 2], second=[], common=True)
SPECS['island_second_contains'] = dict(geometries=['island', 'cross', 'contains_inner'], first=[0], second=[1, 2])
CASES = tuple(SPECS)


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 96):
        raise ValueError('compound intersections require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'compound_intersection' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid compound intersection recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='compound_intersection', id='compound_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('compound intersections require idle execution')
    doc, G = Rhino.RhinoDoc.ActiveDoc, Rhino.Geometry
    if list(doc.Objects):
        raise ValueError('compound intersections require an empty owned document')
    from join_probe import observe_command
    spec, xyz = SPECS[op['case']], host['_xyz']
    owned, ids, layers, groups = [], [], [], []
    saved_layer, saved_tolerance = doc.Layers.CurrentLayerIndex, doc.ModelAbsoluteTolerance
    doc.ModelAbsoluteTolerance = 1e-7

    def keep(g):
        if g is None: raise ValueError('compound source construction failed')
        owned.append(g)
        return g

    def geometry(shape):
        pieces = []
        for bounds, inward in SHAPES[shape]:
            g = keep(G.BoundingBox(G.Point3d(*[p[0] for p in bounds]), G.Point3d(*[p[1] for p in bounds])).ToBrep())
            if inward: g.Flip()
            pieces.append(g)
        result = keep(pieces[0].DuplicateBrep())
        for g in pieces[1:]: result.Append(g)
        if not result.IsValid or not result.IsSolid: raise ValueError('invalid compound source')
        return result

    def record(g):
        area = G.AreaMassProperties.Compute(g)
        mass = G.VolumeMassProperties.Compute(g) if g.IsSolid else None
        try:
            bounds = g.GetBoundingBox(True)
            return dict(valid=bool(g.IsValid), solid=bool(g.IsSolid), orientation=str(g.SolidOrientation),
                        volume=float(mass.Volume) if mass else None, area=float(area.Area) if area else None,
                        centroid=xyz(mass.Centroid) if mass else None,
                        bounds=[xyz(bounds.Min), xyz(bounds.Max)], faces=int(g.Faces.Count), edges=int(g.Edges.Count),
                        vertices=[xyz(v.Location) for v in g.Vertices],
                        edge_samples=[[xyz(e.PointAt(e.Domain.ParameterAt(i/8.))) for i in range(9)] for e in g.Edges],
                        face_regions=[[[xyz(f.PointAt(t.PointAtStart.X, t.PointAtStart.Y)) for t in loop.Trims]
                                       for loop in f.Loops] for f in g.Faces])
        finally:
            if area: area.Dispose()
            if mass: mass.Dispose()

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            g, a = obj.Geometry, obj.Attributes
            if isinstance(g, G.Brep):
                row, kind = record(g), 'Brep'
            elif isinstance(g, G.Extrusion):
                brep = g.ToBrep()
                if brep is None: raise ValueError('compound extrusion conversion failed')
                try: row, kind = record(brep), 'Extrusion'
                finally: brep.Dispose()
            else:
                bounds = g.GetBoundingBox(True)
                kind = str(g.GetType().Name)
                row = dict(valid=bool(g.IsValid), native_geometry_type=str(g.GetType().FullName),
                           bounds=[xyz(bounds.Min), xyz(bounds.Max)])
                if isinstance(g, G.TextDot): row.update(point=xyz(g.Point), text=g.Text)
                elif isinstance(g, G.Point): row['point'] = xyz(g.Location)
                elif isinstance(g, G.Curve):
                    row.update(length=float(g.GetLength()), closed=bool(g.IsClosed),
                               samples=[xyz(g.PointAt(g.Domain.ParameterAt(i/8.))) for i in range(9)])
            row.update(source=ids.index(obj.Id) if obj.Id in ids else None,
                       selected=bool(obj.IsSelected(False)), kind=kind, name=a.Name,
                       layer=layers.index(a.LayerIndex) if a.LayerIndex in layers else None,
                       color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                       groups=sorted(groups.index(i) for i in (a.GetGroupList() or []) if i in groups),
                       attribute_text=a.GetUserString('Code'), geometry_text=g.GetUserString('Code'))
            rows.append(row)
        return rows

    def selectors(indices):
        return ' '.join('_SelID '+str(ids[i]) for i in indices)

    def invoke(macro, name):
        marker = 'Viboceros compound intersection '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success, after, events = observe_command(Rhino.Commands.Command, name,
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        if Rhino.Commands.Command.InCommand(): raise ValueError('compound command remains active')
        return dict(success=success, macro=macro, after=after, after_script=snapshot(), events=events,
                    history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1])

    def clear_objects():
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id, True)

    def sdk_union(gs):
        result = G.Brep.CreateBooleanUnion(gs, 1e-7)
        outputs = [] if result is None else [keep(g) for g in result]
        return dict(returned_null=result is None, outputs=[record(g) for g in outputs])

    try:
        geometries = [geometry(name) for name in spec['geometries']]
        if spec.get('sdk'):
            before = [record(g) for g in geometries]
            result = G.Brep.CreateBooleanIntersection(geometries[0], geometries[1], 1e-7)
            outputs = [] if result is None else [keep(g) for g in result]
            if [record(g) for g in geometries] != before: raise ValueError('SDK mutated compound sources')
            return dict(inputs=before, returned_null=result is None, outputs=[record(g) for g in outputs]), 0
        # Seed DeleteInput before preselection, using only owned dummy objects.
        for name in ('equal_outer', 'cross'): ids.append(doc.Objects.AddBrep(geometry(name)))
        delete = spec.get('delete', True)
        option = '_DeleteInput=_'+('Yes' if delete else 'No')
        seed = '_-BooleanIntersection '+option+' '+selectors([0])+' _Enter '+selectors([1])+' _Enter'
        if not Rhino.RhinoApp.RunScript(seed, False): raise ValueError('compound preference seed failed')
        clear_objects(); ids[:] = []
        first, second = spec['first'], spec['second']
        # Public SDK unions are diagnostics, never used as command inputs.
        source_geometry = [record(g) for g in geometries]
        unions = dict(first=sdk_union([geometries[i] for i in first]),
                      second=sdk_union([geometries[i] for i in second]) if second else None)
        if [record(g) for g in geometries] != source_geometry: raise ValueError('SDK unions mutated source geometry')
        serial = doc.BeginUndoRecord('Compound intersection owned sources')
        if not serial: raise ValueError('compound source undo record failed')
        try:
            for i, g in enumerate(geometries):
                layer, attrs = Rhino.DocObjects.Layer(), Rhino.DocObjects.ObjectAttributes()
                try:
                    layer.Name = 'Viboceros compound '+str(System.Guid.NewGuid())
                    index = doc.Layers.Add(layer)
                    if index < 0: raise ValueError('compound layer failed')
                    layers.append(index)
                    attrs.LayerIndex, attrs.Name = index, 'source-'+str(i)
                    attrs.ObjectColor = System.Drawing.Color.FromArgb(20+i, 40, 60)
                    attrs.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                    attrs.SetUserString('Code', 'attribute-'+str(i)); g.SetUserString('Code', 'geometry-'+str(i))
                    key = doc.Objects.AddBrep(g, attrs)
                    if key == System.Guid.Empty: raise ValueError('compound insertion failed')
                    ids.append(key)
                finally: attrs.Dispose(); layer.Dispose()
            for members in [[key] for key in ids]+[ids]:
                index = doc.Groups.Add('Viboceros compound '+str(System.Guid.NewGuid()), members)
                if index < 0: raise ValueError('compound group failed')
                groups.append(index)
        finally: doc.EndUndoRecord(serial)
        before = snapshot()
        if spec.get('pre'):
            for i in first: doc.Objects.Select(ids[i])
            macro = '_-BooleanIntersection '+selectors(second)+' _Enter'
        else:
            macro = '_-BooleanIntersection '+option+' '+selectors(first)+' _Enter '+selectors(second)+' _Enter'
        result = dict(before=before, first=first, second=second, sdk_unions=unions, command=invoke(macro, 'BooleanIntersection'))
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
