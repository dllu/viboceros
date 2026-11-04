"""Owned actual SelEdgeLoop inputs, verified by the public pick frustum."""
if __package__:
    from .subd_join_probe import source as subd_source
    from .edge_loop_cases import discovery_cases,cases,source,validate
else:
    from subd_join_probe import source as subd_source
    from edge_loop_cases import discovery_cases,cases,source,validate


class Input:
    def __init__(self,op,host,recipe=None):
        self.op=op;self.host=host;self.R=host['Rhino'];self.S=host['System'];self.G=self.R.Geometry
        self.doc=self.R.RhinoDoc.ActiveDoc;self.geometry=None;self.key=None
        self.old_display=self.doc.SubDAppearance;self.mesh=None;self.recipe_override=recipe

    def __enter__(self):
        try:return self.setup()
        except Exception:
            self.__exit__(None,None,None);raise

    def setup(self):
        G,R,S=self.G,self.R,self.S;op=self.op;doc=self.doc
        self.recipe=self.recipe_override if self.recipe_override is not None else source(op['scene']);self.mesh=G.Mesh();mesh=self.mesh;mesh.Vertices.UseDoublePrecisionVertices=True
        for i,p in enumerate(self.recipe['points']):
            mesh.Vertices.Add(G.Point3d(*p))
            if op.get('uv',True):mesh.TextureCoordinates.Add(float(i%5)/4,float(i//5)/3)
        for face in self.recipe['faces']:mesh.Faces.AddFace(*face)
        for n in self.recipe.get('ngons',[]):mesh.Ngons.AddNgon(G.MeshNgon.Create(S.Array[S.UInt32](n['vertices']),S.Array[S.UInt32](n['faces'])))
        mesh.Normals.ComputeNormals()
        self.geometry=mesh.DuplicateMesh() if op['kind']=='mesh' else subd_source(self.recipe,self.host)
        self.key=doc.Objects.Add(self.geometry)
        if self.key==S.Guid.Empty:raise ValueError('cannot insert owned loop source')
        doc.SubDAppearance=G.SubDComponentLocation.ControlNet
        R.RhinoApp.RunScript('_SetView _World _Top',False);R.RhinoApp.RunScript('_Zoom _Extents',False)
        view=doc.Views.ActiveView;viewport=view.ActiveViewport
        mode=R.Display.DisplayModeDescription.FindByName('Wireframe')
        if mode is None:raise ValueError('wireframe mode unavailable')
        viewport.DisplayMode=mode;doc.Views.Redraw()
        self.edges=[];self.native=[]
        xyz=lambda p:(float(p.X),float(p.Y),float(p.Z))
        points={}
        for i,p in enumerate(self.recipe['points']):points.setdefault(tuple(p),i)
        if op['kind']=='mesh':
            for i in range(self.geometry.TopologyEdges.Count):
                line=self.geometry.TopologyEdges.EdgeLine(i)
                self.edges.append([points[xyz(line.From)],points[xyz(line.To)]]);self.native.append(i)
        else:
            vertices=[];vertex=self.geometry.Vertices.First
            while vertex is not None:vertices.append(vertex);vertex=vertex.Next
            lookup={int(v.Id):i for i,v in enumerate(vertices)}
            for edge in self.geometry.Edges:
                self.edges.append([points[xyz(edge.VertexFrom.ControlNetPoint)],points[xyz(edge.VertexTo.ControlNetPoint)]])
                self.native.append(int(edge.ComponentIndex().Index))
        self.seed=next(i for i,pair in enumerate(self.edges) if set(pair)==set(op['pair']))
        a,b=[G.Point3d(*self.recipe['points'][i]) for i in op['pair']]
        self.point=G.Point3d((a.X+b.X)*.5,(a.Y+b.Y)*.5,(a.Z+b.Z)*.5)
        pixel=viewport.WorldToClient(self.point);x,y=int(pixel.X),int(pixel.Y)
        context=R.Input.Custom.PickContext();curve=G.Line(a,b).ToNurbsCurve();references=[]
        try:
            context.View=view;context.PickStyle=R.Input.Custom.PickStyle.PointPick
            context.PickGroupsEnabled=False;context.SubObjectSelectionEnabled=True
            ok,line=viewport.GetFrustumLine(x,y)
            if not ok:raise ValueError('owned loop ray unavailable')
            context.PickLine=line;context.SetPickTransform(viewport.GetPickTransform(S.Drawing.Rectangle(x-6,y-6,12,12)))
            context.UpdateClippingPlanes();references=list(doc.Objects.PickObjects(context) or [])
            if not context.PickFrustumTest(curve)[0] or not any(r.ObjectId==self.key for r in references):
                raise ValueError('owned loop seed missed public pick frustum')
        finally:
            for reference in references:reference.Dispose()
            curve.Dispose();context.Dispose()
        model=R.FileIO.File3dm()
        try:
            if op['kind']=='mesh':model.Objects.AddMesh(self.geometry)
            else:model.Objects.AddSubD(self.geometry)
            self.archive=[int(b) for b in model.ToByteArray()]
        finally:model.Dispose()
        self.crc=int(self.geometry.DataCRC(0));doc.ClearUndoRecords(True)
        return self

    def selected(self):
        obj=self.doc.Objects.FindId(self.key)
        components=[dict(kind=str(c.ComponentIndexType),index=int(c.Index)) for c in (obj.GetSelectedSubObjects() or [])]
        return dict(whole=bool(obj.IsSelected(False)),components=components,ordinals=[self.native.index(c['index']) for c in components])

    def __exit__(self,*error):
        self.R.RhinoApp.RunScript('!',False)
        for obj in list(self.doc.Objects):self.doc.Objects.Delete(obj.Id,True)
        if self.geometry is not None:self.geometry.Dispose()
        if self.mesh is not None:self.mesh.Dispose()
        self.doc.SubDAppearance=self.old_display


def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    with Input(op,host) as value:
        before=value.selected();marker='Owned edge loop '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        from shrink_face_input import drive
        sequence=dict(op,steps=[dict(kind='click',modifiers='plain')],finish='Enter')
        trace=[];success=drive(sequence,[[value.point]],host,value.selected,trace,'SelEdgeLoop')
        after=value.selected();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if not success or not after['components']:raise ValueError('owned loop seed was not accepted: '+history)
        if not R.RhinoApp.RunScript('_-ToNURBS _DeleteInputObjects=_No _Enter',False):raise ValueError('loop conversion failed')
        order=[];points=value.recipe['points']
        for obj in sorted((o for o in doc.Objects if o.Id!=value.key),key=lambda o:o.RuntimeSerialNumber):
            curve=obj.Geometry
            if not isinstance(curve,R.Geometry.NurbsCurve) or curve.Degree!=1:raise ValueError('unexpected flat loop output')
            pair=[min(range(len(points)),key=lambda i:p.DistanceTo(R.Geometry.Point3d(*points[i]))) for p in (curve.PointAtStart,curve.PointAtEnd)]
            order.append(next(i for i,e in enumerate(value.edges) if set(e)==set(pair)))
        if set(order)!=set(after['ordinals']) or len(order)!=len(after['ordinals']):raise ValueError('loop conversion coverage differs')
        return dict(input=value.recipe,archive=value.archive,edges=value.edges,native=value.native,seed=value.seed,
                    before=before,trace=trace,after=after,history=history,to_nurbs_order=order,
                    source_unchanged=int(doc.Objects.FindId(value.key).Geometry.DataCRC(0))==value.crc),0
