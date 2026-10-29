"""Actual selected-owner point-display commands in an owned idle document."""
KINDS = ('line', 'curve', 'rational', 'surface', 'mesh', 'subd')
PATTERNS = ('owner', 'grip', 'both-owners', 'both-grips', 'none', 'peer', 'mixed', 'group-owner', 'group-grip', 'owner-peer', 'off-owner', 'postselect', 'owner-hidden-peer', 'postselect-peer', 'postselect-grip', 'postcancel-peer', 'postcancel-grip', 'postcancel', 'undo-owner', 'undo-none', 'undo-grip', 'command-undo-owner', 'command-undo-none', 'command-undo-grip')


def validate(op):
    if not isinstance(op,dict) or not all(type(value) is str for value in op.values()):
        raise ValueError('fixed selected point-display recipe required')
    fields={'op','id','kind','mode','pattern'}
    if op.get('op')=='pt_off_selected_history':fields.add('control_action')
    if (not isinstance(op,dict) or set(op) != fields
            or op['op'] not in ['pt_off_selected','pt_off_selected_history'] or op['kind'] not in KINDS
            or op['mode'] not in ['control','edit'] or op['pattern'] not in PATTERNS
            or op['mode']=='edit' and op['kind'] in ['surface','mesh']
            or op['id'] != op['kind']+'-'+op['mode']+'-'+op['pattern']+('-'+op['control_action'] if 'control_action' in op else '')):
        raise ValueError('fixed selected point-display recipe required')
    if 'control_action' in op and op['control_action'] not in ['none','PointsOff','PtOffSelected']:
        raise ValueError('fixed history control required')


def create(kind, host):
    G = host['Rhino'].Geometry
    if kind == 'line':
        return G.LineCurve(G.Point3d(0,0,0),G.Point3d(3,4,0))
    if kind in ['curve','rational']:
        points = [[0,0,0],[1,3,1],[3,2,-1],[4,0,0]]
        weights = [1.,2.,0.5,1.] if kind=='rational' else [1.]*4
        return host['_nurbs_curve_from_definition'](dict(degree=3,
            knots=[0.,0.,0.,0.,1.,1.,1.,1.],
            control_points=[dict(point=p,weight=w) for p,w in zip(points,weights)]))
    if kind == 'surface':
        return G.NurbsSurface.CreateFromCorners(G.Point3d(0,0,0),G.Point3d(2,0,0),G.Point3d(2,3,0),G.Point3d(0,3,0))
    mesh = G.Mesh()
    try:
        points = [[0,0,0],[2,0,0],[2,2,0],[0,2,0]]
        faces = [[0,1,2,3]]
        if kind == 'subd':
            points += [[0,0,2],[2,0,2],[2,2,2],[0,2,2]]
            faces = [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]
        for p in points: mesh.Vertices.Add(G.Point3d(*p))
        for f in faces: mesh.Faces.AddFace(*f)
        return G.SubD.CreateFromMesh(mesh) if kind == 'subd' else mesh.DuplicateMesh()
    finally:
        mesh.Dispose()


def run(op, host, iterations, tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned document at idle required')
    ids, groups = [], []
    def state():
        result=[]
        for key in ids:
            obj=doc.Objects.FindId(key)
            result.append(dict(on=bool(obj.GripsOn),selected=bool(obj.IsSelected(False)),
                grips=[dict(index=int(g.Index),point=host['_xyz'](g.CurrentLocation),
                            selected=bool(g.IsSelected(False)),hidden=bool(g.IsHidden))
                       for g in (obj.GetGrips() or [])]))
        return result
    try:
        for _ in range(2):
            geometry=create(op['kind'],host)
            try: key=doc.Objects.Add(geometry)
            finally: geometry.Dispose()
            if key==S.Guid.Empty:raise ValueError('point-display source insertion failed')
            ids.append(key)
        peer=doc.Objects.AddPoint(R.Geometry.Point3d(20,20,20))
        doc.ClearUndoRecords(True);doc.ClearRedoRecords()
        for key in ids:
            if op['mode']=='control':doc.Objects.FindId(key).GripsOn=True
            else:
                doc.Objects.UnselectAll();doc.Objects.Select(key,True)
                R.RhinoApp.RunScript('_EditPtOn',False)
        doc.Objects.UnselectAll()
        for key in ids:
            grips=doc.Objects.FindId(key).GetGrips() or []
            if not grips:raise ValueError('requested point display did not open')
            for g in grips:g.Select(False)
        pattern=op['pattern'][13:] if op['pattern'].startswith('command-undo-') else (op['pattern'][5:] if op['pattern'].startswith('undo-') else op['pattern'])
        command_witness=op['pattern'].startswith('command-undo-')
        if pattern.startswith('group-'):
            groups.append(doc.Groups.Add('Point display peers',S.Array[S.Guid](ids)))
        if pattern=='off-owner':doc.Objects.FindId(ids[0]).GripsOn=False
        if pattern=='owner-hidden-peer':
            doc.Objects.FindId(ids[1]).GetGrips()[1].Select(True)
            R.RhinoApp.RunScript('_HidePt',False)
            if not doc.Objects.FindId(ids[1]).GetGrips()[1].IsHidden:raise ValueError('hidden peer setup failed')
        undo_marker=None
        undo_record=0
        if op['pattern'].startswith('undo-') or command_witness:
            if command_witness:
                R.RhinoApp.RunScript('_Point w90,90,90',False)
                undo_marker=next(obj.Id for obj in doc.Objects if isinstance(obj.Geometry,R.Geometry.Point) and obj.Geometry.Location.DistanceTo(R.Geometry.Point3d(90,90,90))<1e-12)
            else:
                record=doc.BeginUndoRecord('Selected display Undo witness')
                undo_record=int(record)
                try:undo_marker=doc.Objects.AddPoint(R.Geometry.Point3d(90,90,90))
                finally:doc.EndUndoRecord(record)
        # Create the witness after all grip/mode/group setup and prove its branch.
        if undo_marker is not None:
            R.RhinoApp.RunScript('_Undo',False)
            undo_setup_removed=doc.Objects.FindId(undo_marker)is None
            R.RhinoApp.RunScript('_Redo',False)
            undo_setup_restored=doc.Objects.FindId(undo_marker)is not None
        if command_witness:
            R.RhinoApp.RunScript('_Point w99,99,99',False)
            marker=next(obj.Id for obj in doc.Objects if isinstance(obj.Geometry,R.Geometry.Point) and obj.Geometry.Location.DistanceTo(R.Geometry.Point3d(99,99,99))<1e-12)
            redo_record=0
        else:
            record=doc.BeginUndoRecord('Selected point display Redo witness')
            redo_record=int(record)
            try:marker=doc.Objects.AddPoint(R.Geometry.Point3d(99,99,99))
            finally:doc.EndUndoRecord(record)
        R.RhinoApp.RunScript('_Undo',False)
        if doc.Objects.FindId(marker)is not None:raise ValueError('witness Undo failed')
        R.RhinoApp.RunScript('_Redo',False)
        if doc.Objects.FindId(marker)is None:raise ValueError('witness Redo failed')
        R.RhinoApp.RunScript('_Undo',False)
        if doc.Objects.FindId(marker)is not None:raise ValueError('witness second Undo failed')
        doc.Objects.UnselectAll()
        for i,key in enumerate(ids):
            obj=doc.Objects.FindId(key)
            if (pattern in ['owner','group-owner','mixed','owner-peer','off-owner','owner-hidden-peer'] and i==0) or pattern=='both-owners':
                if not obj.Select(True,True,True,True,False,False):raise ValueError('owner selection failed')
            if (pattern in ['grip','group-grip'] and i==0) or pattern=='both-grips' or (pattern in ['mixed','postselect-grip','postcancel-grip'] and i==1):
                if not obj.GetGrips()[0].Select(True):raise ValueError('grip selection failed')
        if pattern in ['peer','owner-peer','postselect-peer','postcancel-peer']:doc.Objects.Select(peer,True)
        marker_text='Selected point display history '+str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker_text)
        before=state()
        sources=[(int(doc.Objects.FindId(key).Geometry.DataCRC(0)),list(doc.Objects.FindId(key).Attributes.GetGroupList() or [])) for key in ids]
        events=[]
        def ended(sender,event):
            if event.CommandEnglishName=='PtOffSelected':events.append(str(event.CommandResult))
        R.Commands.Command.EndCommand += ended
        from hide_pt_input import run as drive
        action=op.get('control_action','PtOffSelected')
        try:
            if action=='none':getter=False
            elif action=='PointsOff':
                R.RhinoApp.RunScript('_PointsOff',False);getter=False
            else:
                _,getter,_=drive(host,cancel=True,command='PtOffSelected',
                                 selection_ids=[ids[0]] if pattern.startswith(('postselect','postcancel')) else (),
                                 accept_selection=not pattern.startswith('postcancel'))
        finally:R.Commands.Command.EndCommand -= ended
        if action=='PtOffSelected' and len(events)!=1:raise ValueError('selected display command did not finish: '+str(events))
        after=state()
        unchanged=sources==[(int(doc.Objects.FindId(key).Geometry.DataCRC(0)),list(doc.Objects.FindId(key).Attributes.GetGroupList() or [])) for key in ids]
        if not unchanged:raise ValueError('point display changed source geometry/groups')
        selected_peer=bool(doc.Objects.FindId(peer).IsSelected(False))
        R.RhinoApp.RunScript('_Redo',False)
        redo=any(isinstance(obj.Geometry,R.Geometry.Point) and obj.Geometry.Location.DistanceTo(R.Geometry.Point3d(99,99,99))<1e-12 for obj in doc.Objects)
        api_redo=False
        if op['op']=='pt_off_selected_history' and not redo:
            api_redo=bool(doc.Redo())
        extra=dict(history=R.RhinoApp.CommandHistoryWindowText.split(marker_text,1)[-1],api_redo=api_redo) if op['op']=='pt_off_selected_history' else {}
        if undo_marker is not None:
            if redo:
                if not doc.Undo() or doc.Objects.FindId(marker)is not None:raise ValueError('Redo witness cleanup failed')
            undo_called=bool(doc.Undo())
            extra.update(undo_record=undo_record,redo_record=redo_record,undo_setup_removed=undo_setup_removed,undo_setup_restored=undo_setup_restored,
                         undo_called=undo_called,undo_witness_removed=doc.Objects.FindId(undo_marker)is None,undo_state=state(),undo_points=[host['_xyz'](obj.Geometry.Location)for obj in doc.Objects if isinstance(obj.Geometry,R.Geometry.Point)],
                         undo_sources_unchanged=sources==[(int(doc.Objects.FindId(key).Geometry.DataCRC(0)),list(doc.Objects.FindId(key).Attributes.GetGroupList() or [])) for key in ids])
            R.RhinoApp.RunScript('_Undo',False)
            extra.update(second_undo_removed=doc.Objects.FindId(undo_marker)is None,second_undo_state=state(),undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker_text,1)[-1])
        return dict(before=before,after=after,peer_selected=selected_peer,unchanged=unchanged,
                    redo_preserved=bool(redo),redo_witness_confirmed=True,getter_opened=getter,
                    command_result=events[0] if events else 'NotInvoked',**extra),0
    finally:
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):
            if obj.GripsOn:obj.GripsOn=False
            doc.Objects.Delete(obj.Id,True)
        for index in groups:doc.Groups.Delete(index)
