"""Fixed public planar-boundary meshing and owned PlanarMesh command cases."""
from copy import deepcopy


def poly(points):
    return dict(kind='polyline',points=deepcopy(points+[points[0]]))


RECT=poly([[0.,0.,0.],[8.,0.,0.],[8.,6.,0.],[0.,6.,0.]])
INNER=poly([[2.,2.,0.],[6.,2.,0.],[6.,4.,0.],[2.,4.,0.]])
CONCAVE=poly([[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[2.,1.,0.],[0.,4.,0.]])
CIRCLE=dict(kind='circle',center=[0.,0.,0.],radius=3.)
CASES={
    'triangle':dict(curves=[poly([[0.,0.,0.],[4.,0.,0.],[0.,3.,0.]])]),
    'rectangle':dict(curves=[RECT]),
    'reverse':dict(curves=[dict(kind='polyline',points=list(reversed(RECT['points'])))]),
    'concave':dict(curves=[CONCAVE]),
    'tilted':dict(curves=[dict(kind='polyline',points=[[x,y,.25*x+.5*y+1.]for x,y,z in CONCAVE['points']])]),
    'circle':dict(curves=[CIRCLE]),
    'ellipse':dict(curves=[dict(kind='ellipse',center=[0.,0.,0.],radii=[4.,2.])]),
    'nurbs_polyline':dict(curves=[dict(kind='nurbs',points=CONCAVE['points'])]),
    'polygon_hole':dict(curves=[RECT,INNER]),
    'hole_reverse':dict(curves=[RECT,dict(kind='polyline',points=list(reversed(INNER['points'])))]),
    'circle_hole':dict(curves=[dict(kind='circle',center=[0.,0.,0.],radius=4.),dict(kind='circle',center=[0.,0.,0.],radius=2.)]),
    'island':dict(curves=[RECT,INNER,poly([[3.,2.5,0.],[5.,2.5,0.],[5.,3.5,0.],[3.,3.5,0.]])]),
    'disconnected':dict(curves=[CONCAVE,dict(kind='polyline',points=[[x+10.,y,z]for x,y,z in RECT['points']])]),
    'overlap':dict(curves=[RECT,dict(kind='polyline',points=[[x+4.,y+3.,z]for x,y,z in RECT['points']])]),
    'different_planes':dict(curves=[RECT,dict(kind='polyline',points=[[x,y,z+2.]for x,y,z in INNER['points']])]),
    'touching_hole':dict(curves=[RECT,poly([[0.,2.,0.],[3.,2.,0.],[3.,4.,0.],[0.,4.,0.]])]),
    'nonplanar':dict(curves=[poly([[0.,0.,0.],[4.,0.,0.],[4.,3.,1.],[0.,3.,0.]])]),
    'mixed_nonplanar':dict(curves=[RECT,poly([[10.,0.,0.],[14.,0.,0.],[14.,3.,1.],[10.,3.,0.]])]),
    'open_chain':dict(curves=[dict(kind='polyline',points=RECT['points'][:-1])]),
    'line_loop':dict(curves=[dict(kind='line',points=[a,b])for a,b in zip(RECT['points'],RECT['points'][1:])]),
    'mixed_open':dict(curves=[RECT,dict(kind='line',points=[[12.,0.,0.],[15.,0.,0.]])]),
}
POST_CASES={name:dict(deepcopy(CASES[source]),postselect=True)for name,source in
            [('post_rectangle','rectangle'),('post_nonplanar','nonplanar'),('post_open_chain','open_chain')]}
SCENES=CASES.copy();SCENES.update(POST_CASES)


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='planar_mesh_command',id=name,case=name)for name in CASES])


def post_request():
    q=request();q['operations']=[dict(op='planar_mesh_command',id=name,case=name)for name in POST_CASES];return q


def validate(op):
    try:
        valid=(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='planar_mesh_command'
               and op['case']in SCENES and op['id']==op['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed PlanarMesh case required')


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;S=host['System']
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case=deepcopy(SCENES[op['case']]);sources=[];curves=[];layers=[];old_layer=doc.Layers.CurrentLayerIndex
    def curve(recipe):
        kind=recipe['kind']
        if kind in ('polyline','nurbs','line'):
            points=[G.Point3d(*p)for p in recipe['points']]
            if kind=='line':return G.LineCurve(points[0],points[1])
            c=G.PolylineCurve(points)
            if kind=='nurbs':
                n=c.ToNurbsCurve();c.Dispose();n.IncreaseDegree(3);return n
            return c
        plane=G.Plane(G.Point3d(*recipe['center']),G.Vector3d.ZAxis)
        if kind=='circle':return G.Circle(plane,recipe['radius']).ToNurbsCurve()
        if kind=='ellipse':return G.Ellipse(plane,*recipe['radii']).ToNurbsCurve()
        raise ValueError('unknown fixed source')
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='source'if obj.Id in sources else'output',source_index=sources.index(obj.Id)if obj.Id in sources else None,
                     selected=bool(obj.IsSelected(False)),layer=doc.Layers[obj.Attributes.LayerIndex].Name,name=obj.Attributes.Name,
                     crc=int(obj.Geometry.DataCRC(0)),color=[int(obj.Attributes.ObjectColor.R),int(obj.Attributes.ObjectColor.G),int(obj.Attributes.ObjectColor.B)],
                     color_source=str(obj.Attributes.ColorSource))
            if isinstance(obj.Geometry,G.Mesh):row['mesh']=host['_polygon_mesh_value'](obj.Geometry)
            rows.append(row)
        return rows
    try:
        curves=[curve(recipe)for recipe in case['curves']]
        parameters=G.MeshingParameters.Default
        fields=['SimplePlanes','JaggedSeams','RefineGrid','GridMinCount','GridMaxCount','RelativeTolerance','Tolerance','MinimumEdgeLength','MaximumEdgeLength']
        parameter_values={name:getattr(parameters,name)for name in fields}
        sdks=[];before_sdk=[int(c.DataCRC(0))for c in curves]
        for c in curves:
            mesh=G.Mesh.CreateFromPlanarBoundary(c,parameters,tolerance['absolute'])
            try:sdks.append(host['_polygon_mesh_value'](mesh)if mesh else None)
            finally:
                if mesh:mesh.Dispose()
        after_sdk=[int(c.DataCRC(0))for c in curves]
        record=doc.BeginUndoRecord('Owned PlanarMesh source')
        try:
            for name in ['PlanarMesh source','PlanarMesh output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            for index,c in enumerate(curves):
                a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Input '+str(index)
                a.ObjectColor=S.Drawing.Color.FromArgb(37,83,149);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
                sources.append(doc.Objects.AddCurve(c,a))
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
            if not case.get('postselect'):
                for identifier in sources:doc.Objects.Select(identifier)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned PlanarMesh '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        command='_-PlanarMesh '
        if case.get('postselect'):command+=' '.join('_SelID '+str(identifier)for identifier in sources)+' '
        command+='_Enter _Cancel'
        success,after,events=observe_command(R.Commands.Command,'PlanarMesh',
            lambda:R.RhinoApp.RunScript(command,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(input=case,sdk_single_boundaries=sdks,sdk_inputs_unchanged=before_sdk==after_sdk,
                    sdk_parameters=parameter_values,document_tolerance=doc.ModelAbsoluteTolerance,
                    success=success,events=events,history=history,before=before,after=after,after_script=after_script,undo=undo,redo=redo),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        for c in curves:c.Dispose()
