"""Real component clicks confined to the owned ExtractPt getter and viewport."""
def run(doc, ids, case, host, macro, identifier):
    from extract_points_component_input import indexes
    from managed_component_input import drive
    R,S=host['Rhino'],host['System']
    G=R.Geometry
    view=doc.Views.ActiveView
    vp=view.ActiveViewport
    doc.Objects.UnselectAll()
    for key in ids:
        obj=doc.Objects.FindId(key);obj.UnselectAllSubObjects()
        for grip in (obj.GetGrips() or []):grip.Select(False)
    host['_record_progress']('ExtractPt getter: prepare component targets')
    plans=[]
    targets=[]
    for source,kind,index in case['components']:
        obj=doc.Objects.FindId(ids[source]);g=obj.Geometry
        component=indexes(g,kind,host)[index]
        aliases=[component]
        if isinstance(g,G.Mesh) and kind=='vertex':aliases.extend(G.ComponentIndex(G.ComponentIndexType.MeshVertex,i) for i in g.TopologyVertices.MeshVertexIndices(index))
        targets.append((obj,aliases))
        bounds=g.GetBoundingBox(True)
        normal=None
        if isinstance(g,G.Mesh):
            if kind=='face':
                face=g.Faces[index]
                polygon=[G.Point3d(g.Vertices[i]) for i in [face.A,face.B,face.C]+([] if face.IsTriangle else [face.D])]
                point=G.Point3d(*[sum(getattr(p,k) for p in polygon)/len(polygon) for k in ['X','Y','Z']])
                normal=G.Vector3d.CrossProduct(polygon[1]-polygon[0],polygon[2]-polygon[0])
            elif kind=='vertex':point=G.Point3d(g.TopologyVertices[index])
            else:point=g.TopologyEdges.EdgeLine(index).PointAt(.5)
        else:
            if kind=='face':
                face=list(g.Faces)[index]
                point=face.ControlNetCenterPoint if case['flat'] else face.LimitSurfaceCenterPoint
                normal=face.ControlNetCenterNormal if case['flat'] else face.SurfaceCenterNormal
            elif kind=='edge':
                edge=list(g.Edges)[index]
                if case['flat']:point=(edge.VertexFrom.ControlNetPoint+edge.VertexTo.ControlNetPoint)*.5
                else:
                    g.UpdateSurfaceMeshCache(False);curve=edge.ToNurbsCurve(True)
                    try:point=curve.PointAt(curve.Domain.Mid)
                    finally:curve.Dispose()
            else:
                vertices=[];vertex=g.Vertices.First
                while vertex is not None:vertices.append(vertex);vertex=vertex.Next
                vertex=vertices[index]
                point=vertex.ControlNetPoint if case['flat'] else vertex.SurfacePoint()
        if normal is None:
            normal=point-bounds.Center
            if bounds.Diagonal.Z < .75*max(bounds.Diagonal.X,bounds.Diagonal.Y):
                normal.Z=(-1. if isinstance(g,G.SubD) else 1.)*max(bounds.Diagonal.Length,1.)
        if not normal.Unitize():raise ValueError('undefined owned component viewing direction')
        plans.append((point,normal,bounds))
    bounds=G.BoundingBox.Empty
    normals=[]
    has_subd=any(isinstance(doc.Objects.FindId(ids[source]).Geometry,G.SubD) for source,kind,index in case['components'])
    for recipe,plan in zip(case['components'],plans):
        bounds.Union(plan[2])
        if not has_subd or isinstance(doc.Objects.FindId(ids[recipe[0]]).Geometry,G.SubD):normals.append(plan[1])
    normal=G.Vector3d(*[sum(getattr(n,k) for n in normals) for k in ['X','Y','Z']])
    if not normal.Unitize():normal=G.Vector3d(0.,0.,-1.)
    plane=G.Plane(bounds.Center,normal)
    if not vp.SetToPlanView(plane.Origin,plane.XAxis,plane.YAxis,True):raise ValueError('could not align owned component viewport')
    vp.DisplayMode=R.Display.DisplayModeDescription.FindByName('Shaded')
    bounds.Inflate(.5);vp.ZoomBoundingBox(bounds);view.Redraw();R.RhinoApp.Wait()
    host['_record_progress']('ExtractPt getter: fixed owned viewport ready')
    views=[]
    for index,plan in enumerate(plans):
        pixel=vp.WorldToClient(plan[0])
        if not 1<=pixel.X<vp.Size.Width-1 or not 1<=pixel.Y<vp.Size.Height-1:raise ValueError('owned component pick outside viewport')
        screen=view.ClientToScreen(S.Drawing.Point(int(pixel.X),int(pixel.Y)))
        views.append((screen,targets[index][0],targets[index][1]))
    selections=' '.join('_SelID '+str(ids[i]) for i in case['whole'])
    script=macro+' '+selections+' _Pause'
    host['_record_progress']('ExtractPt getter: run managed physical selection')
    return drive(host,script,identifier,views,any(s['grips'] for s in case['sources']))
