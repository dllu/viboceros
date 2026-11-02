"""Owned public ExtractAnalysisMesh commands, colors, attributes and history."""
import math
import os
import re

SOURCES = ('plane', 'warped', 'holed', 'box', 'sphere', 'mesh', 'subd_plane', 'subd_box', 'extrusion', 'line', 'block', 'block_multi')
MODES = ('None', 'Draft', 'DraftOff', 'Curvature', 'CurvatureOff')


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op','id','source','mode','preselect','steps'}
            or op['op'] != 'extract_analysis_mesh' or op['source'] not in SOURCES or op['mode'] not in MODES
            or type(op['preselect']) is not bool or not isinstance(op['id'],str)
            or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id']) is None
            or not isinstance(op['steps'],list) or not 1 <= len(op['steps']) <= 8
            or any(step not in ('Extract','Undo','Redo') for step in op['steps'])):
        raise ValueError('invalid analysis-mesh extraction recipe')


def run(op, host, iterations, tolerance):
    validate(op)
    R,S = host['Rhino'],host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('analysis mesh capture requires empty owned idle document')
    draft, curvature = R.ApplicationSettings.DraftAngleAnalysisSettings, R.ApplicationSettings.CurvatureAnalysisSettings
    old_draft,old_curvature = draft.GetCurrentState(),curvature.GetCurrentState()
    old_layer = doc.Layers.CurrentLayerIndex
    source = model = None
    key = S.Guid.Empty
    group = definition = -1
    layers = []
    xyz = lambda v:[float(v.X),float(v.Y),float(v.Z)]

    def mesh_record(m):
        count = m.Vertices.Count
        indices = sorted(set([i*count//min(64,count) for i in range(min(64,count))]+([count-1] if count else [])))
        return dict(vertices=int(count),faces=int(m.Faces.Count),normals=int(m.Normals.Count),colors=int(m.VertexColors.Count),
                    samples=[dict(point=xyz(m.Vertices.Point3dAt(i)),normal=xyz(m.Normals[i]) if i<m.Normals.Count else None,
                                  color=[int(getattr(m.VertexColors[i],k)) for k in ('R','G','B','A')] if i<m.VertexColors.Count else None) for i in indices])

    def attributes(obj):
        a=obj.Attributes
        return dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,color_source=str(a.ColorSource),
                    color=[int(getattr(a.ObjectColor,k)) for k in ('R','G','B','A')],wire_density=int(a.WireDensity),
                    groups=[doc.Groups[i].Name for i in (a.GetGroupList() or [])],user_text=a.GetUserString('analysis-probe'),geometry_text=obj.Geometry.GetUserString('geometry-probe'),selected=bool(obj.IsSelected(False)))

    def state():
        obj=doc.Objects.FindId(key)
        meshes=[] if obj is None else list(obj.GetMeshes(G.MeshType.Analysis) or [])
        return dict(source=attributes(obj) if obj is not None else None,cache=[mesh_record(m) for m in meshes],
                    outputs=[dict(attributes=attributes(o),mesh=mesh_record(o.Geometry)) for o in doc.Objects if o.Id!=key and isinstance(o.Geometry,G.Mesh)])

    try:
        for command in ('DraftAngleAnalysisOff','CurvatureAnalysisOff'):
            R.RhinoApp.RunScript('_'+command,False)
        for name in ('ExtractionSource','ExtractionDestination'):
            layer=R.DocObjects.Layer();layer.Name=name
            index=doc.Layers.Add(layer);layer.Dispose()
            if index<0:raise ValueError('owned analysis layer creation failed')
            layers.append(index)
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Analysis source';a.WireDensity=7
        a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject;a.ObjectColor=S.Drawing.Color.FromArgb(10,20,30)
        group=doc.Groups.Add('Analysis source group');a.AddToGroup(group);a.SetUserString('analysis-probe','preserved')
        if op['source'] in ('plane','block','block_multi'):
            source=G.Brep.CreateFromCornerPoints(G.Point3d(0,0,0),G.Point3d(3,0,0),G.Point3d(3,2,0),G.Point3d(0,2,0),1e-8)
        elif op['source'] in ('mesh','subd_plane','subd_box'):
            mesh=G.Mesh()
            if op['source']=='subd_box':
                points=[(-2,-2,-2),(2,-2,-2),(2,2,-2),(-2,2,-2),(-2,-2,2),(2,-2,2),(2,2,2),(-2,2,2)]
                faces=[(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]
            else:points=[(0,0,0),(3,0,0),(3,2,0),(0,2,0)];faces=[(0,1,2,3)]
            for p in points:mesh.Vertices.Add(*p)
            for f in faces:mesh.Faces.AddFace(*f)
            mesh.Normals.ComputeNormals()
            if op['source'].startswith('subd_'):source=G.SubD.CreateFromMesh(mesh);mesh.Dispose()
            else:source=mesh
        elif op['source']=='line':source=G.LineCurve(G.Point3d.Origin,G.Point3d(3,0,0))
        elif op['source']=='extrusion':
            curve=G.Circle(G.Plane.WorldXY,2).ToNurbsCurve();source=G.Extrusion.Create(curve,5,True);curve.Dispose()
        else:
            model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','brep_direction',op['source']+'.3dm'))
            source=list(model.Objects)[0].Geometry.DuplicateBrep()
        source.SetUserString('geometry-probe','source geometry')
        if op['source']=='block_multi':
            second=source.DuplicateBrep();second.Transform(G.Transform.Translation(0,0,3))
            line=G.LineCurve(G.Point3d.Origin,G.Point3d(1,0,0))
            mesh=G.Mesh();mesh.Vertices.Add(0,0,0);mesh.Vertices.Add(1,0,0);mesh.Vertices.Add(0,1,0);mesh.Faces.AddFace(0,1,2)
            first_attrs=R.DocObjects.ObjectAttributes();first_attrs.LayerIndex=layers[0];first_attrs.Name='First member';first_attrs.WireDensity=2;first_attrs.ColorSource=R.DocObjects.ObjectColorSource.ColorFromParent
            second_attrs=a.Duplicate();second_attrs.Name='Second member';second_attrs.WireDensity=5;second_attrs.RemoveFromAllGroups()
            definition=doc.InstanceDefinitions.Add('Analysis source block','',G.Point3d.Origin,[source,second,line,mesh],[first_attrs,second_attrs,R.DocObjects.ObjectAttributes(),R.DocObjects.ObjectAttributes()])
            second.Dispose();line.Dispose();mesh.Dispose();first_attrs.Dispose();second_attrs.Dispose()
            key=doc.Objects.AddInstanceObject(definition,G.Transform.Translation(5,2,1),a)
        elif op['source']=='block':
            definition=doc.InstanceDefinitions.Add('Analysis source block','',G.Point3d.Origin,[source],[R.DocObjects.ObjectAttributes()])
            key=doc.Objects.AddInstanceObject(definition,G.Transform.Translation(5,2,1),a)
        elif isinstance(source,G.Brep):key=doc.Objects.AddBrep(source,a,None,False,False)
        else:key=doc.Objects.Add(source,a)
        a.Dispose()
        if key==S.Guid.Empty:raise ValueError('analysis source insertion failed')
        crc=int(doc.Objects.FindId(key).Geometry.DataCRC(0))
        if op['mode']!='None':
            command='DraftAngleAnalysis' if op['mode'].startswith('Draft') else 'CurvatureAnalysis'
            doc.Objects.Select(key)
            R.RhinoApp.RunScript('_'+command+' _Enter',False)
            if command=='DraftAngleAnalysis':
                draft.AngleRange=G.Interval(math.radians(-5),math.radians(7));draft.UpDirection=G.Vector3d(0,0,1)
            else:
                settings=curvature.GetCurrentState();settings.Style=curvature.CurvatureStyle.Gaussian;settings.GaussRange=G.Interval(-1,1);curvature.UpdateFromState(settings)
            doc.Views.Redraw();R.RhinoApp.Wait()
            if op['mode'].endswith('Off'):R.RhinoApp.RunScript('_'+command+'Off',False)
        doc.Objects.UnselectAll();doc.ClearUndoRecords(True)
        states=[state()];traces=[]
        for step in op['steps']:
            if step=='Extract':
                doc.Objects.UnselectAll()
                if op['preselect']:doc.Objects.Select(key)
                macro='_ExtractAnalysisMesh'+('' if op['preselect'] else ' _SelID '+str(key)+' _Enter')
                command='ExtractAnalysisMesh'
            else:macro='_'+step;command=step
            success,unused,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript(macro,False),lambda:None,lambda:[],True)
            doc.Views.Redraw();R.RhinoApp.Wait()
            if int(doc.Objects.FindId(key).Geometry.DataCRC(0))!=crc:raise ValueError('mesh extraction changed source')
            states.append(state());traces.append(dict(command=command,success=success,events=[dict(name=e['name'],result=e['result']) for e in events]))
        return dict(states=states,traces=traces),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for command in ('DraftAngleAnalysisOff','CurvatureAnalysisOff'):R.RhinoApp.RunScript('_'+command,False)
        draft.UpdateFromState(old_draft);curvature.UpdateFromState(old_curvature)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        if definition>=0:doc.InstanceDefinitions.Delete(definition,True,True)
        if group>=0:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        if source is not None:source.Dispose()
        if model is not None:model.Dispose()
