"""Observe ConvertExtrusion on shared files, using only public Rhino APIs."""
import os
if __package__:
    from .convert_extrusion_cases import validate
else:
    from convert_extrusion_cases import validate


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','extrusions',op['asset']+'.3dm'))
    if model is None or model.Objects.Count!=1:raise ValueError('shared extrusion unavailable')
    key=peer=S.Guid.Empty;group=-1
    try:
        source=list(model.Objects)[0].Geometry.Duplicate()
        if not isinstance(source,G.Extrusion):raise ValueError('typed extrusion input required')
        source.SetUserString('Geometry','Input geometry')
        layer_index=doc.Layers.FindByFullPath('Extrusion sources',-1)
        if layer_index<0:
            layer=R.DocObjects.Layer();layer.Name='Extrusion sources'
            try:layer_index=doc.Layers.Add(layer)
            finally:layer.Dispose()
        a=R.DocObjects.ObjectAttributes();a.Name='Conversion source';a.LayerIndex=layer_index
        a.ObjectColor=host['System'].Drawing.Color.FromArgb(17,43,91);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        a.SetUserString('Part','Input extrusion')
        try:key=doc.Objects.AddExtrusion(source,a)
        finally:a.Dispose();source.Dispose()
        keys=[key]
        if op.get('pair'):
            obj=doc.Objects.FindId(key);g=obj.Geometry.Duplicate();a=obj.Attributes.Duplicate();a.Name='Conversion source 1'
            try:keys.append(doc.Objects.AddExtrusion(g,a))
            finally:a.Dispose();g.Dispose()
        a=R.DocObjects.ObjectAttributes();a.Name='Peer'
        try:peer=doc.Objects.AddPoint(G.Point3d(20,30,40),a)
        finally:a.Dispose()
        if S.Guid.Empty in (key,peer):raise ValueError('conversion source admission failed')
        if op['grouped']:
            group=doc.Groups.Add('Conversion group',keys+[peer])
            if group<0:raise ValueError('conversion group admission failed')
        def snapshot():
            rows=[];order=[]
            for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
                g=obj.Geometry;a=obj.Attributes
                role='source' if obj.Id==key else 'source1' if obj.Id in keys[1:] else 'peer' if obj.Id==peer else 'copy1' if a.Name=='Conversion source 1' else 'copy'
                order.append(role)
                row=dict(role=role,selected=int(obj.IsSelected(False)),name=a.Name,groups=int(a.GroupCount),
                         layer=doc.Layers[a.LayerIndex].Name,color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                         color_source=int(a.ColorSource),attribute_text=a.GetUserString('Part'),geometry_text=g.GetUserString('Geometry'))
                if isinstance(g,G.Extrusion):
                    row['kind']='extrusion';brep=g.ToBrep()
                elif isinstance(g,G.Brep):row['kind']='brep';brep=g
                elif isinstance(g,G.Point):row.update(kind='point',point=host['_xyz'](g.Location));brep=None
                else:raise ValueError('unexpected converted geometry')
                if brep is not None:
                    try:row['geometry']=host['_interchange_brep_record'](brep)
                    finally:
                        if brep is not g:brep.Dispose()
                rows.append(row)
            return dict(objects=sorted(rows,key=lambda r:r['role']),runtime_order=order)
        if op['preselect']:
            for selected in (list(reversed(keys)) if op.get('reverse') else keys):doc.Objects.Select(selected)
            if op['grouped']:doc.Objects.Select(peer)
        doc.ClearUndoRecords(True)
        before=snapshot();marker='Owned ConvertExtrusion '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        options='' if op['delete'] is None else ' _DeleteInput=_'+('Yes' if op['delete'] else 'No')
        picks='' if op['preselect'] else ''.join(' _SelID '+str(selected) for selected in (list(reversed(keys)) if op.get('reverse') else keys))+' _Enter'
        macro='_-ConvertExtrusion'+picks+options+' _Enter'
        success,after,events=observe_command(R.Commands.Command,'ConvertExtrusion',
                lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',True);undo=snapshot()
        R.RhinoApp.RunScript('_Redo',True);redo=snapshot()
        return dict(success=success,macro=macro,before=before,after=after,after_script=after_script,
                    undo=undo,redo=redo,events=events,history=history),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if group>=0:doc.Groups.Delete(group)
        doc.ClearUndoRecords(True);model.Dispose()
