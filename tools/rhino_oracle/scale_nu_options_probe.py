"""Closed numeric ScaleNU option recipes in an empty owned Rhino document."""
import re

MODES = {
    'world_yes': '_WorldCoordinates=_Yes w1,2,3 2 3 .5',
    'world_yes_no': '_WorldCoordinates=_Yes _WorldCoordinates=_No w1,2,3 2 3 .5',
    'world_twice': '_WorldCoordinates _WorldCoordinates w1,2,3 2 3 .5',
    'rigid_yes': '_Rigid=_Yes w1,2,3 2 -1 .5',
    'rigid_yes_no': '_Rigid=_Yes _Rigid=_No w1,2,3 2 -1 .5',
    'rigid_y': 'w1,2,3 2 _Rigid=_Yes -1 .5',
    'rigid_repeat': '_Rigid=_Yes w1,2,3 2 -1 .5 4 .5 1',
    'remember_rigid': 'w1,2,3 2 -1 .5',
    'remember_world': 'w1,2,3 2 3 .5',
    'rigid_zero': '_Rigid=_Yes w1,2,3 0 1 1',
    'remember_rigid_cancel': 'w1,2,3 2 -1 .5',
    'remember_factors_cancel': 'w1,2,3 _Enter _Enter _Enter',
    'rigid_followup_move': '_Rigid=_Yes w1,2,3 2 -1 .5',
}
SOURCES = ('point', 'rational', 'periodic', 'surface', 'mesh', 'line', 'arc', 'circle', 'mixed')


def validate_request(request):
    if (not isinstance(request, dict) or type(request.get('protocol_version')) is not int
            or request['protocol_version'] != 1 or type(request.get('iterations', 1)) is not int
            or request.get('iterations', 1) != 1 or not isinstance(request.get('operations'), list)
            or not 1 <= len(request['operations']) <= 64):
        raise ValueError('ScaleNU options require bounded protocol 1 recipes')
    seen = set()
    for op in request['operations']:
        if (not isinstance(op, dict) or set(op) != {'op','id','source','plane','mode','copy','selection','groups'}
                or op['op'] != 'scale_nu_options' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None or op['id'] in seen
                or op['source'] not in SOURCES or op['plane'] not in ('world','rotated','tilted')
                or not isinstance(op['mode'], str) or op['mode'] not in MODES
                or type(op['copy']) is not bool or op['selection'] not in ('objects','grips','parent')
                or op['groups'] not in ('none','pair','overlap')
                or op['groups'] != 'none' and op['source'] != 'mixed'
                or op['selection'] != 'objects' and op['source'] not in ('rational','surface','mesh','mixed')
                or op['mode'] == 'rigid_repeat' and not op['copy']):
            raise ValueError('invalid ScaleNU option recipe')
        seen.add(op['id'])


def request():
    rows = [('point',p,m,False,'objects','none') for p in ('rotated','tilted')
            for m in ('world_yes','world_yes_no','world_twice')]
    rows += [(s,'tilted','rigid_yes',False,'objects','none') for s in SOURCES]
    rows += [(s,'rotated','rigid_yes',True,'objects','none') for s in ('rational','surface','mesh','mixed')]
    rows += [('mixed','world','rigid_yes',False,'objects',g) for g in ('pair','overlap')]
    rows += [(s,'tilted','rigid_yes',True,k,'none')
             for s,k in (('rational','grips'),('surface','parent'),('mixed','grips'))]
    rows += [('mixed','tilted',m,c,'objects','none')
             for m,c in (('rigid_y',False),('rigid_yes_no',False),('rigid_repeat',True))]
    rows += [('rational','world','remember_rigid',False,'objects','none'),
             ('point','tilted','remember_world',False,'objects','none'),
             ('mixed','tilted','rigid_yes',True,'objects','overlap')]
    rows += [('mixed','tilted','rigid_yes',True,'objects','pair'),
             ('mesh','world','rigid_zero',True,'objects','none'),
             ('rational','world','rigid_yes',False,'grips','none'),
             ('surface','tilted','rigid_yes',False,'parent','none'),
             ('rational','world','remember_rigid_cancel',False,'objects','none')]
    rows += [('point','world','remember_factors_cancel',False,'objects','none')]
    return dict(protocol_version=1,iterations=1,operations=[
        dict(op='scale_nu_options',id='scale-nu-options-'+str(i),source=s,plane=p,
             mode=m,copy=c,selection=k,groups=g)
        for i,(s,p,m,c,k,g) in enumerate(rows)])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('ScaleNU option history requires idle execution')
    if list(doc.Objects):
        raise ValueError('ScaleNU options require an empty owned document')
    from grip_transform_probe import create_source
    from join_probe import observe_command
    view = doc.Views.ActiveView
    vp = view.ActiveViewport
    original = Rhino.DocObjects.ViewportInfo(vp)
    original_plane = vp.GetConstructionPlane()
    ids, kinds, group_indices = [], [], []
    def snapshot():
        objects = sorted(list(doc.Objects),key=lambda obj:obj.RuntimeSerialNumber)
        rows = []
        for obj in objects:
            geometry = obj.Geometry
            row = dict(role='source' if obj.Id in ids else 'output',name=obj.Attributes.Name,
                       kind=geometry.GetType().Name,selected=bool(obj.IsSelected(False)),
                       grips_on=bool(obj.GripsOn),
                       grips=[dict(index=int(g.Index),point=host['_xyz'](g.CurrentLocation),selected=bool(g.IsSelected(False)))
                              for g in (obj.GetGrips() or [])])
            if isinstance(geometry,Rhino.Geometry.Curve):
                row['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
            elif isinstance(geometry,Rhino.Geometry.Brep):
                row['surface'] = host['_nurbs_surface_definition'](geometry.Surfaces[0].ToNurbsSurface())
            elif isinstance(geometry,Rhino.Geometry.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](geometry)
            elif isinstance(geometry,Rhino.Geometry.Point):
                row['point'] = host['_xyz'](geometry.Location)
            else:
                raise ValueError('unexpected ScaleNU option geometry')
            rows.append(row)
        groups = []
        for index in range(doc.Groups.Count):
            if doc.Groups.IsDeleted(index): continue
            groups.append([i for i,obj in enumerate(objects) if index in list(obj.Attributes.GetGroupList() or [])])
        return dict(objects=rows,groups=groups)
    try:
        if not vp.SetProjection(Rhino.Display.DefinedViewportProjection.Top,'Owned ScaleNU options',False):
            raise ValueError('ScaleNU option view setup failed')
        plane = Rhino.Geometry.Plane.WorldXY
        if op['plane'] == 'rotated':
            plane = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(0.,1.,0.),Rhino.Geometry.Vector3d(-1.,0.,0.))
        elif op['plane'] == 'tilted':
            plane = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(1.,1.,0.),Rhino.Geometry.Vector3d(-1.,1.,2.))
        vp.SetConstructionPlane(plane)
        seed = doc.Objects.AddPoint(host['_point']([1.,1.,1.]))
        doc.Objects.Select(seed)
        seed_macros = ['_ScaleNU _Copy=_No _Rigid=_No _WorldCoordinates=_No w0,0,0 1 1 1']
        if op['mode'] == 'remember_rigid': seed_macros.append('_ScaleNU _Copy=_No _Rigid=_Yes w0,0,0 1 1 1')
        elif op['mode'] == 'remember_rigid_cancel': seed_macros.append('_ScaleNU _Copy=_No _Rigid=_Yes w0,0,0 7 _Cancel')
        elif op['mode'] == 'remember_world': seed_macros.append('_ScaleNU _Copy=_No _Rigid=_No _WorldCoordinates=_Yes w0,0,0 1 1 1')
        elif op['mode'] == 'remember_factors_cancel':
            seed_macros += ['_ScaleNU _Copy=_No _Rigid=_No w0,0,0 2 3 .5','_ScaleNU _Copy=_No _Rigid=_No w0,0,0 7 _Cancel']
        seed_history = Rhino.RhinoApp.CommandHistoryWindowText
        seed_success = []
        for seed_macro in seed_macros:
            succeeded = bool(Rhino.RhinoApp.RunScript(seed_macro,False))
            seed_success.append(succeeded)
            if succeeded != ('_Cancel' not in seed_macro):
                raise ValueError('ScaleNU option seeding failed')
        seed_history = Rhino.RhinoApp.CommandHistoryWindowText[len(seed_history):]
        doc.Objects.Delete(seed,True)
        serial = doc.BeginUndoRecord('ScaleNU option sources')
        try:
            kinds = ['rational','surface','mesh','point'] if op['source'] == 'mixed' else [op['source']]
            for i,kind in enumerate(kinds):
                geometry = Rhino.Geometry.Point(host['_point']([8.,0.,0.])) if kind == 'point' else create_source(kind,host)
                if op['source'] == 'mixed' and kind != 'point':
                    geometry.Transform(Rhino.Geometry.Transform.Translation(Rhino.Geometry.Vector3d(*[[4.,2.,1.],[-2.,3.,4.],[7.,-4.,1.]][i])))
                attrs = Rhino.DocObjects.ObjectAttributes()
                attrs.Name = 'options source '+str(i)
                source = doc.Objects.Add(geometry,attrs)
                if source == System.Guid.Empty: raise ValueError('ScaleNU option source insertion failed')
                ids.append(source)
                geometry.Dispose()
            definitions = [[0,1]] if op['groups'] == 'pair' else [[0,1],[1,3]] if op['groups'] == 'overlap' else []
            for i, members in enumerate(definitions):
                index = doc.Groups.Add('ScaleNU options group '+str(i),[ids[j] for j in members])
                if index < 0: raise ValueError('ScaleNU option group insertion failed')
                group_indices.append(index)
        finally:
            doc.EndUndoRecord(serial)
        if op['selection'] != 'objects':
            owner = doc.Objects.FindId(ids[0])
            owner.GripsOn = True
            for i in (0,2): owner.GetGrips()[i].Select(True)
            if op['selection'] == 'parent': doc.Objects.Select(ids[0])
            for object_id in ids[1:]: doc.Objects.Select(object_id)
        else:
            for object_id in ids: doc.Objects.Select(object_id)
        bounds = []
        for object_id in ids:
            geometry = doc.Objects.FindId(object_id).Geometry
            def box(accurate):
                b = geometry.GetBoundingBox(accurate)
                return dict(min=host['_xyz'](b.Min),max=host['_xyz'](b.Max),center=host['_xyz'](b.Center))
            bounds.append(dict(fast=box(False),tight=box(True)))
        before = snapshot()
        macro = '_ScaleNU _Copy=_'+('Yes ' if op['copy'] else 'No ')+MODES[op['mode']]+(' _Enter' if op['copy'] else '')
        history = Rhino.RhinoApp.CommandHistoryWindowText
        host['_record_progress'](op['id']+' '+macro)
        success,after,events = observe_command(Rhino.Commands.Command,'ScaleNU',lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after_script = snapshot()
        history = Rhino.RhinoApp.CommandHistoryWindowText[len(history):]
        followup = None
        if op['mode'] == 'rigid_followup_move':
            move_macro = '_Move w0,0,0 w1,2,3'
            move_success = bool(Rhino.RhinoApp.RunScript(move_macro,True))
            followup = dict(macro=move_macro,success=move_success,after=snapshot())
        Rhino.RhinoApp.RunScript('_Undo',False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False)
        redo = snapshot()
        result = dict(before=before,after=after,after_script=after_script,undo=undo,redo=redo,
                    events=events,success=success,history=history,macro=macro,seed_macros=seed_macros,seed_success=seed_success,seed_history=seed_history,
                    bounds=bounds,kinds=kinds,
                    plane=dict(origin=host['_xyz'](plane.Origin),x_axis=host['_xyz'](plane.XAxis),y_axis=host['_xyz'](plane.YAxis)))
        if followup is not None: result['followup'] = followup
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn: obj.GripsOn = False
            doc.Objects.Delete(obj.Id,True)
        for index in group_indices: doc.Groups.Delete(index)
        # Copied definitions are owned too; remove their now empty records.
        for index in range(doc.Groups.Count):
            if not doc.Groups.IsDeleted(index): doc.Groups.Delete(index)
        vp.SetViewProjection(original,False)
        vp.SetConstructionPlane(original_plane)
        original.Dispose()
        doc.Views.Redraw()
