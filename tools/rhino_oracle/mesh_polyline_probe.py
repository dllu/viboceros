"""Fixed closed-polyline SDK and command probes in an owned idle document."""
from copy import deepcopy


def closed(points):
    return deepcopy(points + [points[0]])


TRIANGLE = closed([[0., 0., 0.], [4., 0., 0.], [0., 3., 0.]])
QUAD = closed([[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]])
CONCAVE = closed([[0., 0., 0.], [4., 0., 0.], [4., 4., 0.], [2., 1., 0.], [0., 4., 0.]])
CASES = {
    'triangle': dict(polylines=[TRIANGLE]),
    'rectangle': dict(polylines=[QUAD]),
    'reverse': dict(polylines=[list(reversed(QUAD))]),
    'shifted_seam': dict(polylines=[closed(QUAD[2:-1] + QUAD[:2])]),
    'concave': dict(polylines=[CONCAVE]),
    'concave_reverse': dict(polylines=[list(reversed(CONCAVE))]),
    'collinear_edge': dict(polylines=[closed([[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]])]),
    'tilted': dict(polylines=[[[x,y,x*.25+y*.5+1.]for x,y,z in CONCAVE]]),
    'vertical': dict(polylines=[[[2.,x,y]for x,y,z in CONCAVE]]),
    'shallow': dict(polylines=[closed([[0.,0.,0.],[4.,0.,0.],[4.,3.,.01],[0.,3.,0.]])]),
    'folded': dict(polylines=[closed([[0.,0.,0.],[4.,0.,0.],[4.,3.,2.],[0.,3.,0.]])]),
    'saddle': dict(polylines=[closed([[0.,0.,0.],[4.,0.,2.],[4.,3.,0.],[0.,3.,2.]])]),
    'spatial_concave': dict(polylines=[closed([[0.,0.,0.],[4.,0.,1.],[4.,4.,0.],[2.,1.,2.],[0.,4.,0.]])]),
    'disconnected': dict(polylines=[TRIANGLE,[[x+10.,y,z+1.]for x,y,z in QUAD]]),
    'nested': dict(polylines=[QUAD,closed([[1.,1.,0.],[2.,1.,0.],[2.,2.,0.],[1.,2.,0.]])]),
    'degree_one_nurbs': dict(polylines=[CONCAVE],nurbs=True),
    'mixed_open': dict(polylines=[TRIANGLE,QUAD[:-1]]),
    'self_crossing': dict(polylines=[closed([[0.,0.,0.],[4.,3.,0.],[4.,0.,0.],[0.,3.,0.]])]),
    'collinear': dict(polylines=[closed([[0.,0.,0.],[2.,0.,0.],[4.,0.,0.]])]),
    'near_closed': dict(polylines=[QUAD[:-1]+[[0.,2e-7,0.]]]),
    'far_closed': dict(polylines=[QUAD[:-1]+[[0.,2e-3,0.]]]),
}
VARIANTS = {}
for _name,_points in [
    ('saddle_reverse',list(reversed(CASES['saddle']['polylines'][0][:-1]))),
    ('saddle_seam',CASES['saddle']['polylines'][0][1:-1]+CASES['saddle']['polylines'][0][:1]),
    ('high_corner',[[0.,0.,0.],[4.,0.,0.],[4.,3.,8.],[0.,3.,0.]]),
    ('asymmetric_quad',[[0.,0.,0.],[5.,0.,2.],[4.,3.,.5],[0.,3.,1.]]),
    ('asymmetric_reverse',[[0.,3.,1.],[4.,3.,.5],[5.,0.,2.],[0.,0.,0.]]),
    ('concave_quad',[[0.,0.,0.],[4.,0.,0.],[1.,1.,0.],[0.,3.,0.]]),
    ('spatial_concave_quad',[[0.,0.,0.],[4.,0.,1.],[1.,1.,.3],[0.,3.,0.]]),
    ('spatial_hexagon',[[0.,0.,0.],[3.,0.,1.],[4.,2.,0.],[3.,4.,2.],[0.,4.,0.],[-1.,2.,1.]]),
    ('thin_rectangle',[[0.,0.,0.],[20.,0.,0.],[20.,.001,0.],[0.,.001,0.]]),
    ('translated_quad',[[1e7,1e7,2.],[1e7+4.,1e7,2.],[1e7+4.,1e7+3.,4.],[1e7,1e7+3.,2.]]),
]:
    VARIANTS[_name]=dict(polylines=[closed(_points)])
SCENES=CASES.copy();SCENES.update(VARIANTS)


def request():
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
                operations=[dict(op='mesh_polyline_command',id=name,case=name)for name in CASES])


def variant_request():
    q=request()
    q['operations']=[dict(op='mesh_polyline_command',id=name,case=name)for name in VARIANTS]
    return q


def validate(op):
    try:
        valid=(isinstance(op,dict)and set(op)=={'op','id','case'}
               and op['op']=='mesh_polyline_command'and op['case']in SCENES and op['id']==op['case'])
    except TypeError:
        valid=False
    if not valid:
        raise ValueError('fixed MeshPolyline case required')


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case=deepcopy(SCENES[op['case']]);sources=[];curves=[];layers=[]
    old_layer=doc.Layers.CurrentLayerIndex
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='source'if obj.Id in sources else'output',
                     source_index=sources.index(obj.Id)if obj.Id in sources else None,
                     selected=bool(obj.IsSelected(False)),layer=doc.Layers[obj.Attributes.LayerIndex].Name,
                     name=obj.Attributes.Name,crc=int(obj.Geometry.DataCRC(0)),
                     color=[int(obj.Attributes.ObjectColor.R),int(obj.Attributes.ObjectColor.G),int(obj.Attributes.ObjectColor.B)],
                     color_source=str(obj.Attributes.ColorSource))
            if isinstance(obj.Geometry,G.Mesh):row['mesh']=host['_polygon_mesh_value'](obj.Geometry)
            rows.append(row)
        return rows
    try:
        sdks=[]
        for points in case['polylines']:
            polyline=G.Polyline([G.Point3d(*p)for p in points])
            mesh=G.Mesh.CreateFromClosedPolyline(polyline)
            try:sdks.append(host['_polygon_mesh_value'](mesh)if mesh else None)
            finally:
                if mesh:mesh.Dispose()
            curve=G.PolylineCurve(polyline)
            if case.get('nurbs'):
                nurbs=curve.ToNurbsCurve();curve.Dispose();curve=nurbs
            curves.append(curve)
        record=doc.BeginUndoRecord('Owned MeshPolyline source')
        try:
            for name in ['MeshPolyline source','MeshPolyline output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            for index,curve in enumerate(curves):
                attributes=R.DocObjects.ObjectAttributes();attributes.LayerIndex=layers[0]
                attributes.Name='Input '+str(index)
                attributes.ObjectColor=host['System'].Drawing.Color.FromArgb(37,83,149)
                attributes.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
                sources.append(doc.Objects.AddCurve(curve,attributes))
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
            for identifier in sources:doc.Objects.Select(identifier)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned MeshPolyline '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,after,events=observe_command(R.Commands.Command,'MeshPolyline',
            lambda:R.RhinoApp.RunScript('_-MeshPolyline _Enter _Cancel',True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(input=case,sdk=sdks,sdk_signature=str(G.Mesh.CreateFromClosedPolyline.__doc__),
                    document_tolerance=doc.ModelAbsoluteTolerance,success=success,events=events,
                    history=history,before=before,after=after,after_script=after_script,undo=undo,redo=redo),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        for curve in curves:curve.Dispose()
