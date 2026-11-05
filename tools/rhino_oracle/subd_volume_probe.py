"""Actual SubD volume commands and separate public ToBrep mass diagnostics."""
if __package__:
    from .subd_volume_cases import source,validate
    from .subd_to_nurbs_probe import make_source
else:
    from subd_volume_cases import source,validate
    from subd_to_nurbs_probe import make_source


def definition(subd,host):
    vertices=[];vertex=subd.Vertices.First
    while vertex is not None:vertices.append(vertex);vertex=vertex.Next
    index={int(v.Id):i for i,v in enumerate(vertices)}
    return dict(points=[host['_xyz'](v.ControlNetPoint)for v in vertices],vertex_tags=[int(v.Tag)for v in vertices],faces=[[index[int(f.VertexAt(i).Id)]for i in range(f.EdgeCount)]for f in subd.Faces])


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    recipe=source(op['scene']);subd=make_source(recipe,host);mesh=None;replacement=None;replacement_definition=None
    old=(doc.SubDAppearance,doc.ModelDistanceDisplayPrecision,doc.ModelUnitSystem)
    try:
        doc.ModelDistanceDisplayPrecision=7;doc.ModelUnitSystem=R.UnitSystem.Millimeters
        doc.SubDAppearance=G.SubDComponentLocation.ControlNet if op['flat'] else G.SubDComponentLocation.Surface
        constructed=definition(subd,host);key=doc.Objects.AddSubD(subd)
        if key==S.Guid.Empty:raise ValueError('cannot add owned SubD volume source')
        stored=definition(doc.Objects.FindId(key).Geometry,host)
        if op.get('replace_reversed'):
            replacement=doc.Objects.FindId(key).Geometry.Duplicate()
            replacement.EnsurePrivateCopy();replacement.Flip()
            replacement_definition=definition(replacement,host)
            if definition(doc.Objects.FindId(key).Geometry,host)!=stored:raise ValueError('private reversal changed the document source')
            if not doc.Objects.Replace(key,replacement):raise ValueError('cannot replace owned SubD')
            stored=definition(doc.Objects.FindId(key).Geometry,host)
        keys=[key]
        if op['mixed']:
            mesh=G.Mesh.CreateFromBox(G.BoundingBox(G.Point3d(20,0,0),G.Point3d(22,3,4)),1,1,1)
            other=doc.Objects.AddMesh(mesh)
            if other==S.Guid.Empty:raise ValueError('cannot add owned mixed mesh')
            keys.append(other)
        crc=[int(doc.Objects.FindId(k).Geometry.DataCRC(0))for k in keys]
        def snapshot():
            return dict(selected=[bool(doc.Objects.FindId(k).IsSelected(False))for k in keys],objects=len(list(doc.Objects)),points=[host['_xyz'](obj.Geometry.Location)for obj in doc.Objects if isinstance(obj.Geometry,G.Point)])
        doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w40,50,60',False);R.RhinoApp.RunScript('_Undo',False)
        if op['preselect']:
            for k in keys:doc.Objects.Select(k)
        before=snapshot();command=op['command'];marker='Owned SubD volume '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_'+command+' '+(''if op['preselect']else' '.join('_SelID '+str(k)for k in keys))+' _Enter'
        if 'open_confirmation'in op:host['_record_progress']('VOLUME_CONFIRM '+op['id']+' '+op['open_confirmation'])
        success,_,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],trace=True)
        if 'open_confirmation'in op:host['_record_progress']('VOLUME_CONFIRM_DONE '+op['id'])
        if R.Commands.Command.InCommand():raise ValueError('volume left an active getter')
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if command=='VolumeCentroid' and after['points']:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:
            R.RhinoApp.RunScript('_Redo',False);redo=snapshot();R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        diagnostics=[]
        modes=range(5)if op['scene']in('skew_cube','bipyramid7')and command=='Volume'and not op['flat']and not op['mixed']and op['preselect']else(3,)
        for mode in modes:
            option=G.SubDToBrepOptions(False,S.Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,mode));brep=None;mass=None
            try:
                brep=doc.Objects.FindId(key).Geometry.ToBrep(option)
                if brep is None:raise ValueError('native SubD volume conversion failed')
                mass=G.VolumeMassProperties.Compute(brep,True,True,False,False,1e-12,1e-10)
                diagnostics.append(dict(mode=mode,solid=bool(brep.IsSolid),volume=None if mass is None else float(mass.Volume),centroid=None if mass is None else host['_xyz'](mass.Centroid)))
            finally:
                if mass is not None:mass.Dispose()
                if brep is not None:brep.Dispose()
                option.Dispose()
        unchanged=[int(doc.Objects.FindId(k).Geometry.DataCRC(0))for k in keys]==crc
        if not unchanged:raise ValueError('volume command changed its source')
        return dict(input=recipe,constructed=constructed,stored=stored,replacement=replacement_definition,source_unchanged=unchanged,success=success,before=before,after=after,undo=undo,redo=redo,history=history,events=events,diagnostics=diagnostics),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.SubDAppearance,doc.ModelDistanceDisplayPrecision,doc.ModelUnitSystem=old
        if replacement is not None:replacement.Dispose()
        if mesh is not None:mesh.Dispose()
        subd.Dispose()
