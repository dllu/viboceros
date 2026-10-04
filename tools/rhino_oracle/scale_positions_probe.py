"""Closed ScalePositions recipes using public commands in an owned document."""
import re

SOURCES = ('point','rational','periodic','surface','mesh','line','arc','circle','mixed')
INPUTS = {'numeric':'w1,2,3 2 w3,6,5','negative':'w1,2,3 -2 w3,6,5',
          'reference':'w1,2,3 w3,6,5 w5,10,7',
          'offaxis_reference':'w1,2,3 w3,6,5 w7,2,3',
          'identity':'w1,2,3 1 w3,6,5','zero_cancel':'w1,2,3 0 _Cancel',
          'zero_reference_cancel':'w1,2,3 w3,6,5 w1,2,3 _Cancel',
          'remember_zero_reference':'w1,2,3 _Enter _Cancel',
          'repeat_direction':'w1,2,3 2 w3,6,5 w5,2,3',
          'repeat_factor':'w1,2,3 2 3',
          'remember_scalar_cancel':'w1,2,3 _Enter w3,6,5',
          'remember_mode_cancel':'w1,2,3 2 w3,6,5',
          'change_mode_from_3d':'w1,2,3 2',
          'change_mode_from_1d':'w1,2,3 2 w3,6,5'}


def validate_request(request):
    if (not isinstance(request,dict) or type(request.get('protocol_version')) is not int
            or request['protocol_version'] != 1 or type(request.get('iterations',1)) is not int
            or request.get('iterations',1) != 1 or not isinstance(request.get('operations'),list)
            or not 1 <= len(request['operations']) <= 64):
        raise ValueError('ScalePositions requires bounded protocol 1 recipes')
    seen = set()
    for op in request['operations']:
        if (not isinstance(op,dict) or set(op) != {'op','id','source','plane','mode','input','copy','selection','groups'}
                or op['op'] != 'scale_positions' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen
                or op['source'] not in SOURCES or op['plane'] not in ('world','rotated','tilted')
                or op['mode'] not in ('1d','2d','3d') or not isinstance(op['input'],str) or op['input'] not in INPUTS
                or type(op['copy']) is not bool or op['selection'] not in ('objects','grips','parent')
                or op['groups'] not in ('none','pair','overlap')
                or op['groups'] != 'none' and op['source'] != 'mixed'
                or op['selection'] != 'objects' and op['source'] not in ('rational','surface','mesh','mixed')
                or op['input']=='repeat_direction' and (op['mode']!='1d' or not op['copy'])
                or op['input']=='repeat_factor' and (op['mode']=='1d' or not op['copy'])
                or op['input'] in ('remember_scalar_cancel','remember_mode_cancel') and op['mode']!='1d'
                or op['input']=='change_mode_from_3d' and op['mode']!='1d'
                or op['input']=='change_mode_from_1d' and op['mode']=='1d'
                or op['input'] in ('zero_cancel','zero_reference_cancel','remember_zero_reference') and op['copy']):
            raise ValueError('invalid ScalePositions recipe')
        seen.add(op['id'])


def request():
    rows = [('point',p,m,'numeric',False,'objects','none') for p in ('world','rotated','tilted') for m in ('1d','2d','3d')]
    rows += [('rational','world',m,'numeric',True,'objects','none') for m in ('1d','2d','3d')]
    rows += [('mixed','world','3d','numeric',False,'objects',g) for g in ('pair','overlap')]
    rows += [(s,'tilted','3d','numeric',False,k,'none') for s,k in (('rational','grips'),('surface','parent'),('mesh','grips'))]
    rows += [('mixed','tilted','3d','numeric',True,'grips','none'),('rational','world','3d','negative',False,'objects','none')]
    rows += [('point','world',m,'reference',False,'objects','none') for m in ('1d','2d','3d')]
    rows += [(s,'tilted','3d','numeric',False,'objects','none') for s in SOURCES if s!='point']
    rows += [('mixed','tilted','3d','numeric',True,'objects',g) for g in ('pair','overlap')]
    rows += [('rational','tilted',m,'offaxis_reference',False,'objects','none') for m in ('1d','2d','3d')]
    rows += [('rational','world','1d','negative',False,'objects','none')]
    rows += [('rational','world',m,t,False,'objects','none') for t in ('identity','zero_cancel') for m in ('1d','2d','3d')]
    rows += [('rational','tilted','1d','repeat_direction',True,'objects','none')]
    rows += [('mixed','world',m,'repeat_factor',True,'objects','pair') for m in ('2d','3d')]
    rows += [('rational','world','1d',t,False,'objects','none') for t in ('remember_scalar_cancel','remember_mode_cancel')]
    rows += [('surface','tilted','3d','numeric',True,'parent','none')]
    rows += [('rational','world','1d','change_mode_from_3d',False,'objects','none')]
    rows += [('rational','tilted',m,'change_mode_from_1d',False,'objects','none') for m in ('2d','3d')]
    rows += [('rational','world',m,'zero_reference_cancel',False,'objects','none') for m in ('1d','2d','3d')]
    rows += [('rational','world',m,'remember_zero_reference',False,'objects','none') for m in ('1d','2d','3d')]
    return dict(protocol_version=1,iterations=1,operations=[dict(op='scale_positions',id='scale-positions-'+str(i),source=s,plane=p,mode=m,input=t,copy=c,selection=k,groups=g) for i,(s,p,m,t,c,k,g) in enumerate(rows)])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('ScalePositions history requires idle execution')
    if list(doc.Objects):
        raise ValueError('ScalePositions require an empty owned document')
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
            def user_text(value):
                strings = value.GetUserStrings()
                return {str(key):str(strings[key]) for key in strings.AllKeys} if strings is not None else {}
            row['attribute_user_text'] = user_text(obj.Attributes)
            row['geometry_user_text'] = user_text(geometry)
            if isinstance(geometry,Rhino.Geometry.Curve):
                row['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
            elif isinstance(geometry,Rhino.Geometry.Brep):
                row['surface'] = host['_nurbs_surface_definition'](geometry.Surfaces[0].ToNurbsSurface())
            elif isinstance(geometry,Rhino.Geometry.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](geometry)
            elif isinstance(geometry,Rhino.Geometry.Point):
                row['point'] = host['_xyz'](geometry.Location)
            else:
                raise ValueError('unexpected ScalePositions geometry')
            rows.append(row)
        groups = []
        for index in range(doc.Groups.Count):
            if doc.Groups.IsDeleted(index): continue
            groups.append([i for i,obj in enumerate(objects) if index in list(obj.Attributes.GetGroupList() or [])])
        return dict(objects=rows,groups=groups)
    try:
        if not vp.SetProjection(Rhino.Display.DefinedViewportProjection.Top,'Owned ScalePositions',False):
            raise ValueError('ScalePositions view setup failed')
        plane = Rhino.Geometry.Plane.WorldXY
        if op['plane'] == 'rotated':
            plane = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(0.,1.,0.),Rhino.Geometry.Vector3d(-1.,0.,0.))
        elif op['plane'] == 'tilted':
            plane = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(1.,1.,0.),Rhino.Geometry.Vector3d(-1.,1.,2.))
        vp.SetConstructionPlane(plane)
        seed = doc.Objects.AddPoint(host['_point']([1.,1.,1.]))
        doc.Objects.Select(seed)
        seed_mode = '3d' if op['input'] in ('change_mode_from_3d','remember_zero_reference') else '1d' if op['input']=='change_mode_from_1d' else op['mode']
        seed_macros = ['_ScalePositions _Copy=_No _Mode=_'+seed_mode.upper()+' w0,0,0 w1,0,0 w2,0,0']
        if op['input']=='remember_scalar_cancel':
            seed_macros.append('_ScalePositions _Copy=_No w0,0,0 7 _Cancel')
        elif op['input']=='remember_mode_cancel':
            seed_macros.append('_ScalePositions _Copy=_No _Mode=_3D _Cancel')
        elif op['input']=='remember_zero_reference':
            seed_macros.append('_ScalePositions _Copy=_No _Mode=_'+op['mode'].upper()+' w0,0,0 w1,0,0 w0,0,0')
        seed_history = Rhino.RhinoApp.CommandHistoryWindowText
        seed_success = []
        seed_events = []
        for seed_macro in seed_macros:
            succeeded,_,events = observe_command(Rhino.Commands.Command,'ScalePositions',lambda:Rhino.RhinoApp.RunScript(seed_macro,False),snapshot,lambda:[],True)
            seed_success.append(succeeded)
            seed_events.append(events)
            if '_Cancel' not in seed_macro and not succeeded or Rhino.Commands.Command.InCommand():
                raise ValueError('ScalePositions seeding failed: '+Rhino.RhinoApp.CommandHistoryWindowText[len(seed_history):])
        seed_history = Rhino.RhinoApp.CommandHistoryWindowText[len(seed_history):]
        doc.Objects.Delete(seed,True)
        serial = doc.BeginUndoRecord('ScalePositions sources')
        try:
            kinds = ['rational','surface','mesh','point'] if op['source'] == 'mixed' else [op['source']]
            for i,kind in enumerate(kinds):
                geometry = Rhino.Geometry.Point(host['_point']([8.,0.,0.])) if kind == 'point' else create_source(kind,host)
                if op['source'] == 'mixed' and kind != 'point':
                    geometry.Transform(Rhino.Geometry.Transform.Translation(Rhino.Geometry.Vector3d(*[[4.,2.,1.],[-2.,3.,4.],[7.,-4.,1.]][i])))
                attrs = Rhino.DocObjects.ObjectAttributes()
                attrs.Name = 'options source '+str(i)
                attrs.SetUserString('Description','ScalePositions oracle')
                geometry.SetUserString('SourceKind',kind)
                source = doc.Objects.Add(geometry,attrs)
                if source == System.Guid.Empty: raise ValueError('ScalePositions source insertion failed')
                ids.append(source)
                geometry.Dispose()
            definitions = [[0,1]] if op['groups'] == 'pair' else [[0,1],[1,3]] if op['groups'] == 'overlap' else []
            for i, members in enumerate(definitions):
                index = doc.Groups.Add('ScalePositions group '+str(i),[ids[j] for j in members])
                if index < 0: raise ValueError('ScalePositions group insertion failed')
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
        arguments = INPUTS[op['input']]
        if op['mode']!='1d' and op['input'] in ('numeric','negative','identity'):
            arguments = arguments.rsplit(' ',1)[0]
        mode_option = '' if op['input'] in ('remember_mode_cancel','remember_zero_reference') else '_Mode=_'+op['mode'].upper()+' '
        macro = '_ScalePositions _Copy=_'+('Yes ' if op['copy'] else 'No ')+mode_option+arguments+(' _Enter' if op['copy'] else '')
        if op['selection']=='grips' and op['source']!='mixed':
            macro = '_ScalePositions _Cancel'
        history = Rhino.RhinoApp.CommandHistoryWindowText
        host['_record_progress'](op['id']+' '+macro)
        success,after,events = observe_command(Rhino.Commands.Command,'ScalePositions',lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after_script = snapshot()
        history = Rhino.RhinoApp.CommandHistoryWindowText[len(history):]
        Rhino.RhinoApp.RunScript('_Undo',False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False)
        redo = snapshot()
        result = dict(before=before,after=after,after_script=after_script,undo=undo,redo=redo,
                    events=events,success=success,history=history,macro=macro,seed_macros=seed_macros,seed_success=seed_success,seed_events=seed_events,seed_history=seed_history,
                    bounds=bounds,kinds=kinds,
                    plane=dict(origin=host['_xyz'](plane.Origin),x_axis=host['_xyz'](plane.XAxis),y_axis=host['_xyz'](plane.YAxis)))
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
