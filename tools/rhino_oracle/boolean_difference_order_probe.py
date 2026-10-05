"""Closed public BooleanDifference command, metadata, and selection recipes."""
import re

CASES = ('split_y','split_z','split_asymmetric','three_x','cross_xy','cross_xyz','three_x_reversed','coplanar_cutters')

def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 8):
        raise ValueError('BooleanDifference commands require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'boolean_difference_order_command' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['case'] not in CASES):
            raise ValueError('invalid BooleanDifference command recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='boolean_difference_order_command', id='boolean_difference_order_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    G, doc = Rhino.Geometry, Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('BooleanDifference commands require idle execution')
    if list(doc.Objects):
        raise ValueError('BooleanDifference commands require an empty owned document')
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
                raise ValueError('unexpected BooleanDifference document geometry')
            rows.append(row)
        return rows

    def invoke(macro, command='BooleanDifference'):
        marker = 'Viboceros BooleanDifference '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success, after, events = observe_command(Rhino.Commands.Command, command,
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        if Rhino.Commands.Command.InCommand():
            raise ValueError('BooleanDifference command remains active')
        return dict(success=success, macro=macro, after=after, after_script=snapshot(), events=events,
                    history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1])

    def clear_objects():
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)

    def add_sources():
        outer=((0.,4.),)*3 if case.startswith('cross_') else ((0.,3.),(0.,1.),(0.,1.)) if case.startswith('three_x') else ((0.,2.),)*3
        axis=1 if case=='split_y' else 2 if case=='split_z' else 0
        bounds=[(-1.,3.)]*3;bounds[axis]=(.5,1.) if case=='split_asymmetric' else (.75,1.25)
        cuts=[tuple(bounds)]
        if case.startswith('three_x'):
            cuts=[((.5,1.),(-1.,2.),(-1.,2.)),((2.,2.5),(-1.,2.),(-1.,2.))]
        elif case.startswith('cross_'):
            cuts=[((1.,3.),(-1.,5.),(-1.,5.)),((-1.,5.),(1.,3.),(-1.,5.))]
            if case=='cross_xyz':cuts.append(((-1.,5.),(-1.,5.),(1.,3.)))
        if case=='coplanar_cutters':cuts=[((1.,3.),(0.,1.5),(.5,1.5)),((1.,3.),(.5,2.),(.5,1.5))]
        geometries=[box(outer)]+[box(b) for b in cuts]
        serial = doc.BeginUndoRecord('BooleanDifference owned source setup')
        if not serial: raise ValueError('BooleanDifference setup undo record failed')
        try:
            for i, g in enumerate(geometries):
                a = Rhino.DocObjects.ObjectAttributes()
                try:
                    if i >= len(layers):
                        layer = Rhino.DocObjects.Layer()
                        try:
                            layer.Name = 'Viboceros BooleanDifference '+str(System.Guid.NewGuid())
                            index = doc.Layers.Add(layer)
                            if index < 0: raise ValueError('BooleanDifference layer creation failed')
                            layers.append(index)
                        finally: layer.Dispose()
                    a.LayerIndex, a.Name = layers[i], 'source-'+str(i)
                    a.ObjectColor = System.Drawing.Color.FromArgb(20+i, 40, 60)
                    a.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                    a.SetUserString('Code', 'attribute-'+str(i))
                    g.SetUserString('Code', 'geometry-'+str(i))
                    key = doc.Objects.Add(g, a)
                    if key == System.Guid.Empty: raise ValueError('BooleanDifference source insertion failed')
                    ids.append(key)
                finally:
                    a.Dispose(); g.Dispose()
            for members in [[key] for key in ids] + [ids]:
                index = doc.Groups.Add('Viboceros BooleanDifference '+str(System.Guid.NewGuid()), members)
                if index < 0: raise ValueError('BooleanDifference group creation failed')
                groups.append(index)
        finally: doc.EndUndoRecord(serial)

    try:
        if case != 'defaults':
            for bounds in (((100.,102.),)*3,((101.,103.),)*3):
                g=box(bounds)
                try: ids.append(doc.Objects.AddBrep(g))
                finally: g.Dispose()
            delete = 'No' if case in ('pre_delete_no','pre_delete_no_split') else 'Yes'
            seed = '_-BooleanDifference _DeleteInput=_Yes _DeleteCutters=_Yes '+('_DeleteInput=_No ' if delete=='No' else '')+'_SelID '+str(ids[0])+' _Enter _SelID '+str(ids[1])+' _Enter'
            if not Rhino.RhinoApp.RunScript(seed,False): raise ValueError('BooleanDifference preference seed failed')
            clear_objects(); ids[:]=[]
        add_sources(); before=snapshot()
        first = [0,1] if case in ('disjoint_target','overlapping_targets','pre_targets_reverse','targets_reverse') else [0,2] if case=='mixed' else [0]
        second = [2] if case in ('disjoint_target','overlapping_targets','pre_targets_reverse','targets_reverse') else [0] if case=='corner_reverse' else list(range(1,len(ids)))
        if case=='corner_reverse': first=[1]
        if case in ('pre_targets_reverse','targets_reverse'): first.reverse()
        if case=='three_x_reversed': second.reverse()
        select=lambda indices: ' '.join('_SelID '+str(ids[i]) for i in indices)
        pre=case in ('pre_target','pre_targets_reverse','pre_delete_no','pre_delete_no_split','pre_undo_redo')
        delete='No' if case in ('delete_no','delete_no_delete_cutters','remember','delete_no_split') else 'Yes'
        cutters='No' if case in ('keep_cutters','keep_cutters_disjoint','undo_keep_cutters') else 'Yes'
        if pre:
            for i in first: doc.Objects.Select(ids[i])
            macro='_-BooleanDifference '+select(second)+' _Enter'
        else:
            options='' if case=='defaults' else '_DeleteCutters=_Yes _DeleteInput=_No ' if delete=='No' else '_DeleteInput=_Yes '
            second_options='' if case=='defaults' or delete=='No' else '_DeleteCutters=_'+cutters+' '
            macro='_-BooleanDifference '+options+select(first)+' _Enter '+second_options+select(second)+' _Enter'
            if case=='cancel_selection': macro='_-BooleanDifference _Cancel'
            elif case=='cancel_options': macro='_-BooleanDifference _DeleteInput=_No _Cancel'
            elif case=='cancel_picked': macro='_-BooleanDifference '+options+select(first)+' _Cancel'
            elif case=='cancel_cutters': macro='_-BooleanDifference '+options+select(first)+' _Enter _DeleteCutters=_No '+select(second)+' _Cancel'
        result=dict(before=before,command=invoke(macro))
        if case in ('undo_redo','pre_undo_redo','undo_keep_cutters'):
            result['undo']=invoke('_Undo','Undo');result['redo']=invoke('_Redo','Redo')
        if case in ('remember','cancel_options','cancel_cutters'):
            clear_objects();ids[:]=[]
            for index in groups: doc.Groups.Delete(index)
            groups[:]=[];add_sources()
            result['followup_before']=snapshot()
            result['followup']=invoke('_-BooleanDifference '+select([0])+' _Enter '+select(list(range(1,len(ids))))+' _Enter')
        return result,0
    finally:
        clear_objects()
        for index in groups: doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer, True)
        for index in reversed(layers): doc.Layers.Delete(index, True)
        doc.ModelAbsoluteTolerance = saved_tolerance
