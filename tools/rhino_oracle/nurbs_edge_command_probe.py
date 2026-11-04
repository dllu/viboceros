"""Actual ToNURBS edge picks and public SDK curves in an owned document."""
if __package__:
    from .subd_packing_cases import source
    from .subd_to_nurbs_probe import make_source
    from .join_probe import observe_command
else:
    from subd_packing_cases import source
    from subd_to_nurbs_probe import make_source
    from join_probe import observe_command

def discovery_cases():
    return [dict(op='nurbs_edge_command',id=kind+'_'+str(delete),kind=kind,delete=delete)
            for kind in ('mesh','box') for delete in (False,True)]
ASSETS=('variable_both_ends','constant_chain','variable_loop')
def cases():
    rows=[dict(op='nurbs_edge_command',id=kind+'_all',kind=kind,delete=False,all=True)
          for kind in ('grid','grid_smooth','subdivided_box','pentagonal_prism','five_fan','fan6','mesh')]
    rows += [dict(op='nurbs_edge_command',id=kind+'_flat_'+str(flat),kind=kind,delete=False,all=True,flat=flat)
             for kind in ('box','grid_smooth') for flat in (False,True)]
    rows += [dict(op='nurbs_edge_command',id='mixed_'+str(delete)+'_'+str(first),kind='box',delete=delete,whole=True,whole_first=first)
             for delete in (False,True) for first in (False,True)]
    rows += [dict(op='nurbs_edge_command',id=kind+'_cancel',kind=kind,delete=True,cancel=True) for kind in ('mesh','box')]
    rows += [dict(op='nurbs_edge_command',id='soft_'+asset,kind='soft',asset=asset,delete=False,all=True) for asset in ASSETS]
    return rows

def interaction_cases():
    return [dict(op='nurbs_edge_command',id='nested_'+str(flat),kind='box',delete=False,flat=flat,nested=True) for flat in (False,True)]

def validate(op):
    if not isinstance(op,dict) or op not in discovery_cases()+cases()+interaction_cases() or any(type(op[k]) is not bool for k in ('delete','all','flat','whole','whole_first','cancel','nested') if k in op):
        raise ValueError('fixed owned NURBS edge command required')

def run(op,host,iterations,tolerance):
    validate(op);R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or doc.UndoRecordingIsActive or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    keys=[];layers=[];groups=[];old_layer=doc.Layers.CurrentLayerIndex;old_display=doc.SubDAppearance
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;a=obj.Attributes
            row=dict(source=keys.index(obj.Id) if obj.Id in keys else None,kind=g.GetType().Name,
                selected=bool(obj.IsSelected(False)),components=[dict(kind=str(c.ComponentIndexType),index=int(c.Index)) for c in (obj.GetSelectedSubObjects() or [])],
                crc=int(g.DataCRC(0)),name=a.Name,layer=layers.index(a.LayerIndex),
                groups=[groups.index(i) for i in (a.GetGroupList() or [])],
                color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource))
            if isinstance(g,G.SubD):
                model=R.FileIO.File3dm()
                try:model.Objects.AddSubD(g);row['archive']=[int(b) for b in model.ToByteArray()]
                finally:model.Dispose()
            elif isinstance(g,G.Mesh):row['mesh']=host['_polygon_mesh_value'](g)
            elif isinstance(g,G.Curve):
                curve=g.ToNurbsCurve()
                try:row['curve']=host['_nurbs_curve_definition'](curve)
                finally:curve.Dispose()
            rows.append(row)
        return rows
    try:
        for name in ('source','current'):layers.append(doc.Layers.Add('NURBS edges '+name,S.Drawing.Color.Blue))
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        for name in ('all','first','empty'):groups.append(doc.Groups.Add('NURBS edges '+name))
        if op['kind']=='mesh':
            g=G.Mesh();g.Vertices.UseDoublePrecisionVertices=True
            for p in [[0.,0.,0.],[2.,0.,1.],[4.,0.,0.],[0.,3.,0.],[2.,3.,2.],[4.,3.,0.]]:g.Vertices.Add(G.Point3d(*p))
            g.Faces.AddFace(0,1,4,3);g.Faces.AddFace(1,2,5,4);g.Normals.ComputeNormals()
        elif op['kind']=='soft':
            import os
            model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','subd_soft_nurbs_inputs',op['asset']+'.3dm'))
            if model is None:raise ValueError('cannot read owned soft input')
            try:
                candidates=[o.Geometry for o in model.Objects if isinstance(o.Geometry,G.SubD) and o.Attributes.Name=='Input 0']
                if len(candidates)!=1:raise ValueError('ambiguous owned soft input')
                g=candidates[0].Duplicate()
            finally:model.Dispose()
        else:
            g=make_source(source(op['kind']),host)
            if op['kind']=='subdivided_box' and not g.Subdivide(1):raise ValueError('cannot subdivide owned source')
        try:
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input 0';a.AddToGroup(groups[0]);a.AddToGroup(groups[1])
            a.ObjectColor=S.Drawing.Color.FromArgb(17,43,91);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
            try:keys.append(doc.Objects.Add(g,a))
            finally:a.Dispose()
        finally:g.Dispose()
        if keys[0]==S.Guid.Empty:raise ValueError('cannot insert source')
        if op.get('whole'):
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input 1';a.AddToGroup(groups[0]);a.ObjectColor=S.Drawing.Color.FromArgb(18,44,92);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
            try:keys.append(doc.Objects.AddLine(G.Point3d(20,0,0),G.Point3d(24,1,0),a))
            finally:a.Dispose()
            if op.get('whole_first'):doc.Objects.Select(keys[1])
        if 'flat' in op:doc.SubDAppearance=G.SubDComponentLocation.ControlNet if op['flat'] else G.SubDComponentLocation.Surface
        obj=doc.Objects.FindId(keys[0]);g=obj.Geometry;sdk=[];components=[]
        count=g.TopologyEdges.Count if isinstance(g,G.Mesh) else g.Edges.Count
        selected=list(reversed(range(count))) if op.get('all') else [3,0]
        if isinstance(g,G.SubD):
            g.UpdateSurfaceMeshCache(False);edges=list(g.Edges)
            for i in selected:
                edge=edges[i];components.append(edge.ComponentIndex())
                curves=[]
                for clamp in (False,True):
                    c=edge.ToNurbsCurve(clamp)
                    try:curves.append(host['_nurbs_curve_definition'](c))
                    finally:c.Dispose()
                sdk.append(dict(edge=i,component=int(edge.ComponentIndex().Index),curves=curves))
        else:
            for i in selected:
                components.append(G.ComponentIndex(G.ComponentIndexType.MeshTopologyEdge,i));c=g.TopologyEdges.EdgeLine(i).ToNurbsCurve()
                try:sdk.append(dict(edge=i,component=i,curves=[host['_nurbs_curve_definition'](c)]))
                finally:c.Dispose()
        for c in components:
            if not obj.SelectSubObject(c,True,True,False):raise ValueError('cannot select source edge')
        if op.get('whole') and not op.get('whole_first'):doc.Objects.Select(keys[1])
        doc.ClearUndoRecords(True);before=snapshot();marker='Owned ToNURBS edges '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-ToNURBS _DeleteInputObjects=_'+('Yes' if op['delete'] else 'No')+(" '_SubDDisplayToggle" if op.get('nested') else '')+(' !' if op.get('cancel') else ' _Enter')
        trace=[];completion=[]
        action=lambda:R.RhinoApp.RunScript(macro,True)
        success,after,events=observe_command(R.Commands.Command,'ToNURBS',action,snapshot,lambda:[],True)
        if R.Commands.Command.InCommand():raise ValueError('edge macro left command active')
        settled=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(before=before,after=after,settled=settled,undo=undo,redo=redo,sdk=sdk,selected=selected,
            history=history,trace=trace,completion=completion,success=success,results=[e['result'] for e in events if e['name']=='ToNURBS']),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.SubDAppearance=old_display
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for group in reversed(groups):doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
