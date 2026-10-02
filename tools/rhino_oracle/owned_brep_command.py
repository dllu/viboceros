"""Owned B-rep command fixtures and complete native document observations."""


class OwnedBrepCommand:
    def __init__(self, host):
        self.host=host;self.Rhino=host['Rhino'];self.System=host['System']
        self.doc=self.Rhino.RhinoDoc.ActiveDoc
        self.settings=self.Rhino.DocObjects.ObjectEnumeratorSettings()
        self.settings.NormalObjects=self.settings.HiddenObjects=self.settings.LockedObjects=True
        self.original=set(obj.Id for obj in self.objects())
        self.selection=[obj.Id for obj in self.objects() if obj.IsSelected(False)]
        self.ids=[];self.groups=[];self.owned=[]

    def objects(self): return list(self.doc.Objects.GetObjectList(self.settings))

    def geometry(self, geometry):
        if isinstance(geometry,self.Rhino.Geometry.Brep):
            return dict(type='brep',definition=self.host['_interchange_brep_record'](geometry,include_samples=False),untrimmed=[bool(face.IsSurface) for face in geometry.Faces])
        if isinstance(geometry,self.Rhino.Geometry.Curve):
            return dict(type='curve',definition=self.host['_nurbs_curve_definition'](geometry))
        raise ValueError('unexpected component command geometry')

    def setup(self, sources):
        self.doc.Objects.UnselectAll();constructed=[]
        for index,source in enumerate(sources):
            path=source['brep']['artifact_path']
            if path.startswith('/'):path='Z:'+path.replace('/','\\')
            model=self.Rhino.FileIO.File3dm.Read(path)
            if model is None:raise ValueError('cannot read owned component source')
            try:
                entries=list(model.Objects)
                if len(entries)!=1:raise ValueError('component source requires one object')
                geometry=entries[0].Geometry.Duplicate()
            finally:model.Dispose()
            self.owned.append(geometry)
            if not isinstance(geometry,self.Rhino.Geometry.Brep) or not geometry.IsValid:
                raise ValueError('invalid component command B-rep')
            constructed.append(self.geometry(geometry))
            attributes=self.Rhino.DocObjects.ObjectAttributes()
            try:
                attributes.Name='source-%d'%index
                attributes.ObjectColor=self.System.Drawing.Color.FromArgb(10+index,30,50)
                attributes.ColorSource=self.Rhino.DocObjects.ObjectColorSource.ColorFromObject
                key=self.doc.Objects.AddBrep(geometry,attributes,None,False,False)
            finally:attributes.Dispose()
            if key==self.System.Guid.Empty:raise ValueError('component source insertion failed')
            self.ids.append(key)
            group=self.doc.Groups.Add('Viboceros component '+str(self.System.Guid.NewGuid()),[key])
            if group<0:raise ValueError('component source grouping failed')
            self.groups.append(group)
        return constructed

    def snapshot(self):
        result=[]
        for obj in sorted((obj for obj in self.objects() if obj.Id not in self.original),key=lambda obj:obj.RuntimeSerialNumber):
            attributes=obj.Attributes
            result.append(dict(source=self.ids.index(obj.Id) if obj.Id in self.ids else None,selected=bool(obj.IsSelected(False)),
                name=attributes.Name,color=[int(attributes.ObjectColor.R),int(attributes.ObjectColor.G),int(attributes.ObjectColor.B)],
                color_source=str(attributes.ColorSource),current_layer=attributes.LayerIndex==self.doc.Layers.CurrentLayerIndex,
                groups=sorted(self.groups.index(group) for group in (attributes.GetGroupList() or []) if group in self.groups),geometry=self.geometry(obj.Geometry)))
        return result

    def components(self):
        kinds={'BrepEdge':'edge','BrepFace':'face'};result=[]
        for source,key in enumerate(self.ids):
            obj=self.doc.Objects.FindId(key)
            if obj is None:continue
            for component in (obj.GetSelectedSubObjects() or []):
                result.append([source,kinds[str(component.ComponentIndexType)],int(component.Index)])
        return sorted(result)

    def verify_edge_pick(self, source, edge_index, view, viewport, x, y):
        """Confirm the owned object and intended edge in the public pick frustum.

        This read-only check does not establish command eligibility: a command
        can ignore a correctly targeted edge. Use before the first edit, since
        subsequent replacement can change the source's numeric edge indices.
        """
        Rhino,System=self.Rhino,self.System
        obj=self.doc.Objects.FindId(self.ids[source])
        if obj is None:raise ValueError('owned edge pick source missing')
        context=Rhino.Input.Custom.PickContext();references=[];curve=None
        try:
            context.View=view;context.PickStyle=Rhino.Input.Custom.PickStyle.PointPick
            context.PickGroupsEnabled=False;context.SubObjectSelectionEnabled=True
            ok,line=viewport.GetFrustumLine(x,y)
            if not ok:raise ValueError('owned edge pick ray unavailable')
            context.PickLine=line
            context.SetPickTransform(viewport.GetPickTransform(System.Drawing.Rectangle(x-8,y-8,16,16)))
            context.UpdateClippingPlanes()
            references=list(self.doc.Objects.PickObjects(context) or [])
            if not any(reference.ObjectId==obj.Id for reference in references):
                raise ValueError('public picker missed owned edge pick source')
            curve=obj.Geometry.Edges[edge_index].ToNurbsCurve()
            if curve is None or not context.PickFrustumTest(curve)[0]:
                raise ValueError('intended edge missed public pick frustum')
        finally:
            if curve is not None:curve.Dispose()
            for reference in references:reference.Dispose()
            context.Dispose()

    def verify_face_pick(self, source, face_index, view, viewport, x, y, shaded=True):
        """Verify a ray/face intersection, optionally with a shaded object hit.

        This verifies the input location; a command can still reject that face.
        Wireframe can certify the ray without requiring cached render meshes.
        No selection or geometry is changed, and every reference is released.
        """
        Rhino,System=self.Rhino,self.System
        obj=self.doc.Objects.FindId(self.ids[source])
        if obj is None:raise ValueError('owned face pick source missing')
        if type(face_index) is not int or not 0<=face_index<obj.Geometry.Faces.Count:
            raise ValueError('owned face pick index outside source')
        resources=[]
        try:
            ok,line=viewport.GetFrustumLine(x,y)
            if not ok:raise ValueError('owned face pick ray unavailable')
            if shaded:
                context=Rhino.Input.Custom.PickContext();resources.append(context)
                context.View=view;context.PickStyle=Rhino.Input.Custom.PickStyle.PointPick
                context.PickMode=Rhino.Input.Custom.PickMode.Shaded
                context.PickGroupsEnabled=False;context.SubObjectSelectionEnabled=True
                context.PickLine=line
                context.SetPickTransform(viewport.GetPickTransform(System.Drawing.Rectangle(x-4,y-4,8,8)))
                context.UpdateClippingPlanes()
                references=list(self.doc.Objects.PickObjects(context) or [])
                resources.extend(references)
                if not any(reference.ObjectId==obj.Id for reference in references):
                    raise ValueError('public shaded picker missed owned face source')
            ray=Rhino.Geometry.LineCurve(line)
            resources.append(ray)
            ok,curves,points=Rhino.Geometry.Intersect.Intersection.CurveBrepFace(
                ray,obj.Geometry.Faces[face_index],self.doc.ModelAbsoluteTolerance)
            resources.extend(curves or [])
            if not ok or not points:
                raise ValueError('public pick ray missed intended owned face')
        finally:
            failures=[]
            for resource in reversed(resources):
                try:resource.Dispose()
                except Exception as error:failures.append(str(error))
            if failures:raise ValueError('owned face pick cleanup failed: '+str(failures))

    def __enter__(self):return self

    def __exit__(self, *exception):
        errors=[]
        def cleanup(action):
            try:action()
            except Exception as error:errors.append(str(error))
        cleanup(lambda:self.Rhino.RhinoApp.RunScript('!',False))
        created=[]
        cleanup(lambda:created.extend(obj for obj in self.objects() if obj.Id not in self.original))
        for obj in created:cleanup(lambda obj=obj:self.doc.Objects.Delete(obj.Id,True))
        for group in self.groups:cleanup(lambda group=group:self.doc.Groups.Delete(group))
        cleanup(self.doc.Objects.UnselectAll)
        for key in self.selection:cleanup(lambda key=key:self.doc.Objects.Select(key))
        for geometry in reversed(self.owned):cleanup(geometry.Dispose)
        if errors:raise ValueError('component command cleanup failed: '+str(errors))
