"""Actual Area/AreaCentroid results and separate public conversion diagnostics."""
if __package__:
    from .subd_area_cases import source,validate
    from .subd_to_nurbs_probe import make_source
else:
    from subd_area_cases import source,validate
    from subd_to_nurbs_probe import make_source


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    recipe=source(op['scene']);subd=make_source(recipe,host)
    old=(doc.SubDAppearance,doc.ModelDistanceDisplayPrecision,doc.ModelUnitSystem)
    try:
        doc.ModelDistanceDisplayPrecision=7;doc.ModelUnitSystem=R.UnitSystem.Millimeters
        doc.SubDAppearance=G.SubDComponentLocation.ControlNet if op['flat'] else G.SubDComponentLocation.Surface
        key=doc.Objects.AddSubD(subd)
        if key==S.Guid.Empty:raise ValueError('cannot add owned SubD area source')
        crc=int(doc.Objects.FindId(key).Geometry.DataCRC(0))
        curve=doc.Objects.AddCircle(G.Circle(G.Point3d(20,0,0),2.)) if op.get('curve') else None
        curve_crc=None if curve is None else int(doc.Objects.FindId(curve).Geometry.DataCRC(0))
        def snapshot():
            return dict(selected=bool(doc.Objects.FindId(key).IsSelected(False)),objects=len(list(doc.Objects)),points=[host['_xyz'](obj.Geometry.Location)for obj in doc.Objects if isinstance(obj.Geometry,G.Point)])
        commands=[]
        for command in ('Area','AreaCentroid'):
            doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
            R.RhinoApp.RunScript('_Point w40,50,60',False);R.RhinoApp.RunScript('_Undo',False)
            if op.get('preselect',True):
                doc.Objects.Select(key)
                if curve is not None:doc.Objects.Select(curve)
            before=snapshot();marker='Owned SubD area '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
            macro='_'+command+' '+('' if op.get('preselect',True) else '_SelID '+str(key)+' ')+' _Enter'
            success,_,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],trace=True)
            if R.Commands.Command.InCommand():raise ValueError('area left an active getter')
            after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
            if command=='AreaCentroid' and after['points']:
                R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot();R.RhinoApp.RunScript('_Undo',False)
            else:
                R.RhinoApp.RunScript('_Redo',False);redo=snapshot();R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
            commands.append(dict(command=command,success=success,before=before,after=after,undo=undo,redo=redo,history=history,events=events))
        diagnostics=[]
        modes=range(5) if op['scene']in('cube','raised_fan','fan7','asymmetric8','pentagon')else(0,3)
        for mode in modes:
            option=G.SubDToBrepOptions(False,S.Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,mode));brep=None;mass=None
            try:
                brep=subd.ToBrep(option)
                if brep is None:raise ValueError('native SubD conversion failed')
                mass=G.AreaMassProperties.Compute(brep,True,True,False,False,1e-12,1e-10)
                diagnostics.append(dict(mode=mode,name=str(S.Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,mode)),area=float(mass.Area),centroid=host['_xyz'](mass.Centroid)))
            finally:
                if mass is not None:mass.Dispose()
                if brep is not None:brep.Dispose()
                option.Dispose()
        unchanged=int(doc.Objects.FindId(key).Geometry.DataCRC(0))==crc
        unchanged=unchanged and (curve is None or int(doc.Objects.FindId(curve).Geometry.DataCRC(0))==curve_crc)
        if not unchanged:raise ValueError('area command changed its SubD source')
        return dict(input=recipe,commands=commands,diagnostics=diagnostics,source_unchanged=unchanged),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.SubDAppearance,doc.ModelDistanceDisplayPrecision,doc.ModelUnitSystem=old
        subd.Dispose()
