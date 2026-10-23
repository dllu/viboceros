"""Fixed public Crease/RemoveCrease command recipes in an owned idle document."""
from copy import deepcopy
if __package__:
    from .crease_cases import DISCOVERY, MAIN, EXTRA, POLICY, MIXED, OVERLAP, ALL_CASES
else:
    from crease_cases import DISCOVERY, MAIN, EXTRA, POLICY, MIXED, OVERLAP, ALL_CASES


def request(main=False,extra=False,policy=False,mixed=False,overlap=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='crease_command',id=k,case=k)for k in (OVERLAP if overlap else MIXED if mixed else POLICY if policy else EXTRA if extra else MAIN if main else DISCOVERY)])


def validate(op):
    try:
        valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='crease_command'and op['case']in ALL_CASES and op['id']==op['case']
    except TypeError:
        valid=False
    if not valid:
        raise ValueError('fixed crease command recipe required')


def run(op,host,iterations,tolerance):
    validate(op)
    return run_scene(ALL_CASES[op['case']],host,iterations,tolerance,op['case']in DISCOVERY)


def run_scene(recipe,host,iterations,tolerance,discovery=False):
    from join_probe import observe_command
    from subd_box_probe import subd_value
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case=deepcopy(recipe);ids=[];layers=[];group=-1
    def vertices(g):
        result=[];v=g.Vertices.First
        while v is not None:
            result.append(v);v=v.Next
        return result
    def snapshot():
        result=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;a=obj.Attributes
            row=dict(grips_on=bool(obj.GripsOn),selected_grips=[int(grip.Index)for grip in(obj.GetGrips()or[])if grip.IsSelected(False)]if obj.GripsOn else[],source=ids.index(obj.Id)if obj.Id in ids else None,selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,groups=[doc.Groups.GroupName(i)for i in (a.GetGroupList()or[])],color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],color_source=int(a.ColorSource),crc=int(g.DataCRC(0)))
            if isinstance(g,G.SubD):
                vv=vertices(g);ee=list(g.Edges)
                row['subd']=subd_value(g,host)
                row['selected_vertices']=[i for i,v in enumerate(vv)if obj.IsSubObjectSelected(v.ComponentIndex())]
                row['selected_edges']=[i for i,e in enumerate(ee)if obj.IsSubObjectSelected(e.ComponentIndex())]
                row['public_edge_members']=[str(p.Name)for p in ee[0].GetType().GetProperties()]if ee else[]
            elif isinstance(g,G.Point):
                row['point']=host['_xyz'](g.Location)
            else:
                row['mesh']=host['_polygon_mesh_value'](g)
                row['mesh']['vertex_normals']=[[float(n.X),float(n.Y),float(n.Z)]for n in g.Normals]
                row['mesh']['colors']=[[int(c.R),int(c.G),int(c.B),int(c.A)]for c in g.VertexColors]if g.VertexColors.Count else None
                row['mesh']['ngons']=[dict(vertices=[int(i)for i in n.BoundaryVertexIndexList()],faces=[int(i)for i in n.FaceIndexList()])for n in g.Ngons]
                if case.get('mesh_channels'):row['mesh']['uv']=[[float(t.X),float(t.Y)]for t in g.TextureCoordinates]
                row['selected_edges']=[i for i in range(g.TopologyEdges.Count)if obj.IsSubObjectSelected(G.ComponentIndex(G.ComponentIndexType.MeshTopologyEdge,i))]
            result.append(row)
        return result
    try:
        undo=doc.BeginUndoRecord('Owned crease setup')
        try:
            for name,color in [('Crease input',S.Drawing.Color.Blue),('Crease current',S.Drawing.Color.Green)]:
                layers.append(doc.Layers.Add(name,color))
            group=doc.Groups.Add('Crease assembly')
            for index,d in enumerate(case['sources']):
                m=G.Mesh();m.Vertices.UseDoublePrecisionVertices=True
                for p in d['vertices']:m.Vertices.Add(G.Point3d(*p))
                for f in d['faces']:m.Faces.AddFace(*f)
                for color in d.get('colors',[]):m.VertexColors.Add(S.Drawing.Color.FromArgb(color[3],color[0],color[1],color[2]))
                for ngon in d.get('ngons',[]):m.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](ngon['vertices']),S.Array[S.UInt32](ngon['faces'])))
                m.Normals.ComputeNormals()
                for uv in d.get('texture_coordinates',[]):m.TextureCoordinates.Add(float(uv[0]),float(uv[1]))
                for i,n in enumerate(d.get('normals',[])):m.Normals.SetNormal(i,G.Vector3f(*n))
                a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input '+str(index);a.ObjectColor=S.Drawing.Color.FromArgb(17,43,91);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.AddToGroup(group)
                if d['kind']=='subd':
                    g=G.SubD.CreateFromMesh(m)
                    if g is None:raise ValueError('cannot construct source SubD')
                    vv=vertices(g);mapping={int(v.Id):i for i,v in enumerate(vv)}
                    for i in d.get('corners',[]):vv[i].Tag=G.SubDVertexTag.Corner
                    for edge in g.Edges:
                        if sorted([mapping[int(edge.VertexFrom.Id)],mapping[int(edge.VertexTo.Id)]])in d.get('creases',[]):edge.Tag=G.SubDEdgeTag.Crease
                    g.UpdateAllTagsAndSectorCoefficients();key=doc.Objects.AddSubD(g,a);g.Dispose()
                else:key=doc.Objects.AddMesh(m,a)
                m.Dispose();ids.append(key)
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
        finally:doc.EndUndoRecord(undo)
        def select_components(seed=False):
            for key,d in zip(ids,case['sources']):
                obj=doc.Objects.FindId(key);g=obj.Geometry
                if case.get('whole'):
                    if case.get('preselected'):doc.Objects.Select(key)
                    continue
                if isinstance(g,G.SubD):
                    vv=vertices(g);mapping={int(v.Id):i for i,v in enumerate(vv)}
                    edge_pairs=d.get('seed_edges',[])if seed else d.get('edges',[])
                    seen_pairs=set()
                    if case.get('grips'):
                        if case.get('edit_points'):
                            obj.Select(True);R.RhinoApp.RunScript('_EditPtOn',False)
                        else:obj.GripsOn=True
                        grips=list(obj.GetGrips()or[])
                        for i in d.get('selected_vertices',[]):grips[i].Select(True)
                    else:
                        for i in d.get('selected_vertices',[]):
                            if not obj.SelectSubObject(vv[i].ComponentIndex(),True,True,False):raise ValueError('vertex preselection failed')
                    for edge_index,edge in enumerate(g.Edges):
                        pair=sorted([mapping[int(edge.VertexFrom.Id)],mapping[int(edge.VertexTo.Id)]])
                        match=tuple(pair)if pair in edge_pairs else None
                        if case.get('edges_by_position')and not seed:
                            points=sorted([host['_xyz'](edge.VertexFrom.ControlNetPoint),host['_xyz'](edge.VertexTo.ControlNetPoint)])
                            match=next((tuple(sorted(p))for p in edge_pairs if points==sorted([d['vertices'][p[0]],d['vertices'][p[1]]])),None)
                        if 'selected_edge_indices'in d and not seed:match=(edge_index,)if edge_index in d['selected_edge_indices']else None
                        if match is not None and(not case.get('one_edge_per_pair')or match not in seen_pairs):
                            if not obj.SelectSubObject(edge.ComponentIndex(),True,True,False):raise ValueError('edge preselection failed')
                            seen_pairs.add(match)
                else:
                    if seed:continue
                    for i in d.get('selected_vertices',[]):
                        if not obj.SelectSubObject(G.ComponentIndex(G.ComponentIndexType.MeshTopologyVertex,i),True,True,False):raise ValueError('mesh vertex preselection failed')
                    for i in range(g.TopologyEdges.Count):
                        pair=g.TopologyEdges.GetTopologyVertices(i)
                        a=list(g.TopologyVertices.MeshVertexIndices(pair.I));b=list(g.TopologyVertices.MeshVertexIndices(pair.J))
                        if any(sorted([x,y])in d.get('edges',[])for x in a for y in b):
                            if not obj.SelectSubObject(G.ComponentIndex(G.ComponentIndexType.MeshTopologyEdge,i),True,True,False):raise ValueError('mesh edge preselection failed')
        if not discovery:
            if case.get('seed_unweld'):
                R.RhinoApp.RunScript('_-UnweldEdge _ModifyNormals=_Yes _Enter',False)
                select_components(seed=True)
                seeded=[]
                def seed_ended(sender,event):
                    if event.CommandEnglishName=='UnweldEdge':seeded.append(str(event.CommandResult))
                R.Commands.Command.EndCommand+=seed_ended
                try:R.RhinoApp.RunScript('_-UnweldEdge _Enter _Cancel _Cancel',False)
                finally:R.Commands.Command.EndCommand-=seed_ended
                if seeded!=['Success']:raise ValueError('edge cut seed failed: '+str(seeded))
                for o in doc.Objects:o.UnselectAllSubObjects();o.Select(False)
            for key,d in zip(ids,case['sources']):
                if 'offset_vertices'not in d:continue
                g=doc.Objects.FindId(key).Geometry.Duplicate()
                try:
                    vv=vertices(g);delta=d['offset']
                    for i in d['offset_vertices']:
                        p=vv[i].ControlNetPoint;vv[i].ControlNetPoint=G.Point3d(p.X+delta[0],p.Y+delta[1],p.Z+delta[2])
                    g.UpdateAllTagsAndSectorCoefficients()
                    if not doc.Objects.Replace(key,g):raise ValueError('offset source replacement failed')
                finally:g.Dispose()
            for key,d in zip(ids,case['sources']):
                if not d.get('soft'):continue
                obj=doc.Objects.FindId(key);g=obj.Geometry;vv=vertices(g);mapping={int(v.Id):i for i,v in enumerate(vv)}
                for edge in g.Edges:
                    if sorted([mapping[int(edge.VertexFrom.Id)],mapping[int(edge.VertexTo.Id)]])in d.get('edges',[]):obj.SelectSubObject(edge.ComponentIndex(),True,True,False)
                seeded=[]
                def ended(sender,event):
                    if event.CommandEnglishName=='SubDCrease':seeded.append(str(event.CommandResult))
                R.Commands.Command.EndCommand+=ended
                try:R.RhinoApp.RunScript('_-SubDCrease '+str(d['soft'])+' _Enter _Cancel _Cancel',True)
                finally:R.Commands.Command.EndCommand-=ended
                if seeded!=['Success']:raise ValueError('soft crease seed failed: '+str(seeded))
                for o in doc.Objects:o.UnselectAllSubObjects();o.Select(False)
            doc.ClearUndoRecords(True)
            witness_record=doc.BeginUndoRecord('Owned crease Undo witness')
            try:
                attrs=R.DocObjects.ObjectAttributes();attrs.Name='Undo witness';attrs.LayerIndex=layers[1]
                doc.Objects.AddPoint(G.Point3d(19.,23.,29.),attrs)
            finally:doc.EndUndoRecord(witness_record)
        else:doc.ClearUndoRecords(True)
        if 'modify_normals'in case:
            R.RhinoApp.RunScript('_-'+case['command']+' _ModifyNormals=_'+('Yes'if case['modify_normals']else 'No')+' _Enter',False)
        if case.get('prime_arguments'):
            R.RhinoApp.RunScript('_-'+case['command']+' '+case['prime_arguments']+' _Enter _Cancel _Cancel',False)
        if not case.get('postselect'):select_components()
        if case.get('soft_vertex_only'):
            for obj in doc.Objects:
                if isinstance(obj.Geometry,G.SubD):
                    for e in obj.Geometry.Edges:obj.SelectSubObject(e.ComponentIndex(),False,True,False)
        source_3dm=None
        if not discovery:
            model=R.FileIO.File3dm()
            try:
                source_layer=model.Layers.AddLayer('Default',S.Drawing.Color.Black)
                for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
                    # The source file carries exact geometry, including hidden
                    # SubD sharpness. Captured attributes are replayed separately.
                    attrs=obj.Attributes.Duplicate();attrs.LayerIndex=source_layer
                    attrs.RemoveFromAllGroups();model.Objects.Add(obj.Geometry,attrs)
                model.Settings.ModelAbsoluteTolerance=doc.ModelAbsoluteTolerance
                model.Settings.ModelUnitSystem=doc.ModelUnitSystem
                source_3dm=S.Convert.ToBase64String(model.ToByteArray())
            finally:model.Dispose()
        before=snapshot();macro='_-'+case['command']
        if case.get('command_arguments'):macro+=' '+case['command_arguments']
        if case.get('whole')and not case.get('preselected'):
            for key in ids:macro+=" '_-SelID "+str(key)
        if not case.get('postselect'):
            macro+=' _Cancel'if case.get('cancel')else' _Enter'
            macro+=' _Cancel _Cancel'
        marker='Owned crease '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        timer=None;tick_errors=[];ticks=[0]
        if case.get('postselect'):
            from System.Windows.Forms import Timer
            timer=Timer();timer.Interval=100
            def tick(sender,event):
                ticks[0]+=1
                try:
                    if ticks[0]==1:
                        if not case.get('cancel'):select_components()
                        R.RhinoApp.SendKeystrokes('_Cancel'if case.get('cancel')else'_Enter',True)
                    elif ticks[0]>=20:
                        timer.Stop();tick_errors.append('postselection did not finish');R.RhinoApp.SendKeystrokes('_Cancel',True)
                except Exception as error:
                    timer.Stop();tick_errors.append(str(error));R.RhinoApp.SendKeystrokes('_Cancel',True)
            timer.Tick+=tick;timer.Start()
        input_trace=[]
        if case.get('live'):
            from conic_getter_input import drive
            view=doc.Views.ActiveView
            view.ActiveViewport.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned '+case['command']+' input',False)
            bounds=G.BoundingBox.Empty
            for key in ids:bounds.Union(doc.Objects.FindId(key).Geometry.GetBoundingBox(True))
            view.ActiveViewport.ZoomBoundingBox(bounds);view.Redraw()
            def execute():
                operation=dict(op='insert_point_command',id=case['capture_id'],actions=case['actions'])
                if 'view_target_offset_z'in case:operation['view_target_offset_z']=case['view_target_offset_z']
                if case.get('capture_pick_hint'):operation['capture_pick_hint']=True
                result,trace=drive(operation,host,[],command=case['command']);input_trace.append(trace);return result
        else:execute=lambda:R.RhinoApp.RunScript(macro,True)
        try:success,after,events=observe_command(R.Commands.Command,case['command'],execute,snapshot,lambda:[],True)
        finally:
            if timer is not None:timer.Stop();timer.Tick-=tick;timer.Dispose()
        if tick_errors:raise ValueError(str(tick_errors))
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        after_script=snapshot();R.RhinoApp.RunScript('_Undo',False);undone=snapshot();R.RhinoApp.RunScript('_Redo',False);redone=snapshot()
        return dict(source_3dm=source_3dm,input=case,macro=macro,input_trace=input_trace,before=before,success=success,events=events,after=after,after_script=after_script,history=history,undo=undone,redo=redone),0
    finally:
        doc.Layers.SetCurrentLayerIndex(0,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if group>=0:doc.Groups.Delete(group)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
