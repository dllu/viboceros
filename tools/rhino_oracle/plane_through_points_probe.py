"""Owned PlaneThroughPt commands on point objects, clouds and selected grips."""
import json
import math
import os


def request():
    with open(os.path.join(os.path.dirname(__file__), 'fixtures', 'plane_through_points.json')) as stream:
        return json.load(stream)


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'points', 'source', 'postselect'}
            or op['op'] != 'plane_through_points'
            or op['source'] not in ('points', 'cloud', 'mesh_grips', 'curve_grips')
            or type(op['postselect']) is not bool
            or not isinstance(op['points'], list) or not 1 <= len(op['points']) <= 256
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(v) not in (int, float) or math.isnan(v) or math.isinf(v) for v in p)
                   for p in op['points'])):
        raise ValueError('invalid PlaneThroughPt recipe')
    if op['source'].endswith('_grips') and (len(op['points']) < 3 or op['postselect']):
        raise ValueError('grip recipes require at least three preselected grips')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('PlaneThroughPt commands require one iteration')
    R=host['Rhino']; G=R.Geometry; doc=R.RhinoDoc.ActiveDoc
    ids=[];created=[];owned=[]
    saved_layer=doc.Layers.CurrentLayerIndex
    source_layer=doc.Layers.Add('PlaneFitSource', host['System'].Drawing.Color.Red)
    current_layer=doc.Layers.Add('PlaneFitCurrent', host['System'].Drawing.Color.Blue)
    if source_layer<0 or current_layer<0: raise ValueError('private fit layers failed')
    doc.Layers.SetCurrentLayerIndex(current_layer, True)
    try:
        points=[host['_point'](p) for p in op['points']]
        attr=R.DocObjects.ObjectAttributes();attr.LayerIndex=source_layer;attr.Name='fit-source'
        attr.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        attr.ObjectColor=host['System'].Drawing.Color.Red
        if op['source']=='points':
            ids=[doc.Objects.AddPoint(p,attr) for p in points]
        elif op['source']=='cloud':
            cloud=G.PointCloud(points);owned.append(cloud)
            ids=[doc.Objects.AddPointCloud(cloud,attr)]
        elif op['source']=='mesh_grips':
            mesh=G.Mesh();owned.append(mesh)
            for p in points:mesh.Vertices.Add(p)
            mesh.Faces.AddFace(0,1,2)
            ids=[doc.Objects.AddMesh(mesh,attr)]
        else:
            curve=G.NurbsCurve(3,False,2,len(points));owned.append(curve)
            for i,p in enumerate(points):curve.Points.SetPoint(i,p)
            curve.Knots.CreateUniformKnots(1.)
            ids=[doc.Objects.AddCurve(curve,attr)]
        if any(i==host['System'].Guid.Empty for i in ids):raise ValueError('fit input creation failed')
        doc.Objects.UnselectAll()
        if op['source'].endswith('_grips'):
            obj=doc.Objects.FindId(ids[0]);obj.GripsOn=True
            for grip in obj.GetGrips():grip.Select(True)
        elif not op['postselect']:
            for i in ids:doc.Objects.Select(i,True)
        macro='_-PlaneThroughPt'
        if op['postselect']:
            macro+=' '+' '.join('_SelID '+str(i) for i in ids)+' _Enter'
        before=set(o.Id for o in doc.Objects)
        serial=doc.BeginUndoRecord('PlaneThroughPt oracle')
        try:ran=R.RhinoApp.RunScript(macro,False)
        finally:
            if serial:doc.EndUndoRecord(serial)
        created=[o.Id for o in doc.Objects if o.Id not in before]
        outputs=[]
        for i in created:
            obj=doc.Objects.FindId(i);geo=obj.Geometry
            surface=geo.Surfaces[0] if isinstance(geo,G.Brep) else geo
            n=surface.ToNurbsSurface()
            try:
                outputs.append(dict(definition=host['_nurbs_surface_definition'](n),
                    corners=[host['_xyz'](n.PointAt(n.Domain(0).ParameterAt(u),n.Domain(1).ParameterAt(v)))
                             for u,v in [(0,0),(1,0),(0,1),(1,1)]],
                    layer=doc.Layers[obj.Attributes.LayerIndex].Name,name=obj.Attributes.Name or None,
                    selected=bool(obj.IsSelected(False)),groups=list(obj.Attributes.GetGroupList() or []),
                    color_source=str(obj.Attributes.ColorSource)))
            finally:n.Dispose()
        source_states=[]
        for i in ids:
            obj=doc.Objects.FindId(i)
            source_states.append(dict(present=obj is not None,selected=bool(obj.IsSelected(False)) if obj else False,
                grips_selected=sum(bool(g.IsSelected(False)) for g in (obj.GetGrips() or [])) if obj and obj.GripsOn else 0))
        return dict(command_returned=bool(ran),outputs=outputs,sources=source_states),0
    finally:
        R.RhinoApp.RunScript('!',False);doc.Objects.UnselectAll()
        for i in ids:
            obj=doc.Objects.FindId(i)
            if obj:obj.GripsOn=False
        for i in created+ids:doc.Objects.Delete(i,True)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for i in [source_layer,current_layer]:doc.Layers.Delete(i,True)
        for geometry in owned:geometry.Dispose()
