"""Public SimplifyCrv commands with independent SDK controls and shared sources."""
import math,re
FAMILIES=('line','nurbs_line','near_line','arc','nurbs_arc','nurbs_circle','ellipse',
          'polyline','polyline_collinear','polycurve_lines','polycurve_arcs','mixed_polycurve',
          'rational_constant','cubic','fully_multiple','nonfull_multiple','near_tangent','polyline_line','polyline_backtrack','polyline_closed','mixed_arc_run','polycurve_nurbs_line','degree_one','weighted_degree_one')
def validate(op):
    if (not isinstance(op,dict) or set(op)-{'domain','delta','mixed','copies','preselect'}!={'op','id','family','artifact_path'}
        or op['op']!='simplify_curve_command' or op['family']not in FAMILIES
        or not isinstance(op['id'],str)or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id'])is None
        or not isinstance(op['artifact_path'],str)or not op['artifact_path'].endswith('.3dm')):
        raise ValueError('invalid SimplifyCrv recipe')
    if 'domain'in op:
        d=op['domain']
        if not isinstance(d,list)or len(d)!=2 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in d)or not d[0]<d[1]:raise ValueError('invalid source domain')
    d=op.get('delta',1e-8)
    if type(d)not in(int,float)or math.isnan(d)or math.isinf(d)or not 0<=d<=.1:raise ValueError('invalid source perturbation')
    if type(op.get('mixed',False))is not bool or type(op.get('preselect',True))is not bool:raise ValueError('invalid input flags')
    if type(op.get('copies',1))is not int or not 1<=op.get('copies',1)<=3:raise ValueError('invalid bounded source count')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('SimplifyCrv capture requires one iteration')
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if R.Commands.Command.InCommand()or list(doc.Objects):raise ValueError('SimplifyCrv requires an owned empty idle document')
    owned=[];layers=[];groups=[];attrs=[]
    saved_layer=doc.Layers.CurrentLayerIndex
    def keep(g):
        if g is None or not g.IsValid:raise ValueError('invalid SimplifyCrv source')
        owned.append(g);return g
    def nurbs(points,knots=None,weights=None):
        curve=keep(host['_nurbs_curve_from_definition'](dict(degree=3,control_points=[dict(point=p,weight=w)for p,w in zip(points,weights or [1.]*len(points))],knots=knots or [0.]*4+[1.]*4)))
        if weights is None:curve.Points.MakeNonRational()
        return curve
    def line(a,b):return keep(G.LineCurve(G.Point3d(*a),G.Point3d(*b)))
    def arc(a,b):
        return keep(G.ArcCurve(G.Arc(G.Circle(G.Plane.WorldXY,2.),G.Interval(a,b))))
    def poly(parts):
        p=G.PolyCurve();owned.append(p)
        for part in parts:
            if not p.Append(part):raise ValueError('polycurve append failed')
        if not p.IsValid:raise ValueError('invalid source polycurve')
        return p
    def record_curve(c):
        value=host['_interchange_curve_record'](c)
        if isinstance(c,G.NurbsCurve):value['rational']=bool(c.IsRational)
        if isinstance(c,G.ArcCurve):
            a=c.Arc;value['arc']=dict(center=host['_xyz'](a.Center),x=host['_xyz'](a.Plane.XAxis),normal=host['_xyz'](a.Plane.ZAxis),radius=float(a.Radius),angles=[float(a.StartAngle),float(a.EndAngle)])
        if isinstance(c,G.PolylineCurve):
            value['vertices']=[host['_xyz'](c.Point(i))for i in range(c.PointCount)]
            n=c.ToNurbsCurve()
            try:value['parameters']=host['_nurbs_curve_definition'](n)['knots'][1:-1]
            finally:n.Dispose()
        if isinstance(c,G.PolyCurve):value['segments']=[record_curve(c.SegmentCurve(i))for i in range(c.SegmentCount)]
        return value
    def snapshot():
        objects=[]
        for o in doc.Objects:
            a=o.Attributes;g=o.Geometry
            data=g.GetUserStrings();text=dict((str(k),str(data[k]))for k in data.AllKeys)if data else{}
            strings=a.GetUserStrings();strings=dict((str(k),str(strings[k]))for k in strings.AllKeys)if strings else{}
            objects.append(dict(id=str(o.Id),name=str(a.Name)if a.Name else None,layer=int(a.LayerIndex),color_source=str(a.ColorSource),color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],groups=list(int(i)for i in(a.GetGroupList()or[])),user_text=strings,geometry_text=text,selected=bool(o.IsSelected(False)),geometry=record_curve(g)if isinstance(g,G.Curve)else dict(type='point',point=host['_xyz'](g.Location))))
        return objects
    try:
        family=op['family'];d=op.get('delta',1e-8)
        if family=='line':source=line([0.,0.,0.],[6.,0.,0.])
        elif family in('nurbs_line','near_line'):source=nurbs([[0.,0.,0.],[1.,d if family=='near_line'else 0.,0.],[4.,-d if family=='near_line'else 0.,0.],[6.,0.,0.]])
        elif family in('arc','nurbs_arc','nurbs_circle'):
            source=arc(0.,math.pi*2. if family=='nurbs_circle'else math.pi*.75)
            if family!='arc':source=keep(source.ToNurbsCurve())
        elif family in('degree_one','weighted_degree_one'):
            source=keep(host['_nurbs_curve_from_definition'](dict(degree=1,control_points=[dict(point=p,weight=w)for p,w in zip([[0.,0.,0.],[2.,0.,0.],[5.,0.,0.],[5.,6.,0.]],[.5,2.,3.,1.]if family=='weighted_degree_one'else[1.]*4)],knots=[0.,0.,1.,2.,3.,3.])))
            if family=='degree_one':source.Points.MakeNonRational()
        elif family=='ellipse':source=keep(G.Ellipse(G.Plane.WorldXY,3.,1.).ToNurbsCurve())
        elif family in('polyline_line','polyline_backtrack','polyline_closed'):
            pts={'polyline_line':[[0.,0.,0.],[3.,0.,0.],[6.,0.,0.]],'polyline_backtrack':[[0.,0.,0.],[3.,0.,0.],[1.,0.,0.],[5.,0.,0.]],'polyline_closed':[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],[0.,0.,0.]]}[family]
            source=keep(G.PolylineCurve(S.Array[G.Point3d]([G.Point3d(*p)for p in pts])))
        elif family=='mixed_arc_run':source=poly([arc(0.,math.pi*.5),arc(math.pi*.5,math.pi),line([-2.,0.,0.],[-5.,0.,0.])])
        elif family=='polycurve_nurbs_line':source=poly([nurbs([[0.,0.,0.],[1.,0.,0.],[4.,0.,0.],[6.,0.,0.]]),line([6.,0.,0.],[6.,3.,0.])])
        elif family in('polyline','polyline_collinear'):
            pts=[[0.,0.,0.],[2.,0.,0.],[5.,0.,0.],[5.,3.,0.],[5.,6.,0.]]if family=='polyline_collinear'else[[0.,0.,0.],[3.,0.,0.],[3.,4.,0.]]
            source=keep(G.PolylineCurve(S.Array[G.Point3d]([G.Point3d(*p)for p in pts])))
        elif family=='polycurve_lines':source=poly([line([0.,0.,0.],[2.,0.,0.]),line([2.,0.,0.],[6.,0.,0.]),line([6.,0.,0.],[6.,3.,0.])])
        elif family=='polycurve_arcs':source=poly([arc(0.,math.pi*.5),arc(math.pi*.5,math.pi)])
        elif family=='mixed_polycurve':source=poly([line([2.,-3.,0.],[2.,0.,0.]),arc(0.,math.pi*.5),line([0.,2.,0.],[-3.,2.,0.])])
        elif family=='near_tangent':source=poly([line([2.+d,-3.,0.],[2.,0.,0.]),arc(0.,math.pi*.5)])
        elif family in('rational_constant','cubic'):
            source=nurbs([[0.,0.,0.],[1.,3.,0.],[3.,-1.,0.],[5.,2.,0.]],weights=[7.]*4 if family=='rational_constant'else None)
        elif family=='fully_multiple':source=nurbs([[0.,0.,0.],[1.,1.,0.],[2.,1.,0.],[3.,0.,0.],[4.,3.,0.],[5.,-1.,0.],[6.,0.,0.]],[0.]*4+[.5]*3+[1.]*4)
        else:source=nurbs([[0.,0.,0.],[1.,1.,0.],[2.,1.,0.],[4.,3.,0.],[5.,-1.,0.],[6.,0.,0.]],[0.]*4+[.5]*2+[1.]*4)
        if 'domain'in op:source.Domain=G.Interval(*op['domain'])
        source.SetUserString('GeometryCode','source')
        layer=R.DocObjects.Layer();layer.Name='SimplifySource_'+str(S.Guid.NewGuid())
        try:index=doc.Layers.Add(layer)
        finally:layer.Dispose()
        if index<0:raise ValueError('source layer creation failed')
        layers.append(index)
        a=R.DocObjects.ObjectAttributes();attrs.append(a);a.Name='SimplifySource';a.LayerIndex=index;a.SetUserString('ObjectCode','source');a.ObjectColor=S.Drawing.Color.FromArgb(17,47,87);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        ids=[]
        for copy in range(op.get('copies',1)):
            placed=keep(source.DuplicateCurve())
            if copy and not placed.Transform(G.Transform.Translation(20.*copy,0.,0.)):raise ValueError('source placement failed')
            key=doc.Objects.AddCurve(placed,a)
            if key==S.Guid.Empty:raise ValueError('curve insertion failed')
            ids.append(key)
        source_ids=list(ids)
        if op.get('mixed'):ids.append(doc.Objects.AddPoint(G.Point3d(20.,20.,0.)))
        group=doc.Groups.Add('SimplifyGroup_'+str(S.Guid.NewGuid()),S.Array[S.Guid](ids));groups.append(group)
        if group<0:raise ValueError('source group failed')
        if op.get('preselect',True):
            for key in ids:doc.Objects.Select(key)
        control=source.Simplify(G.CurveSimplifyOptions.All,tolerance['absolute'],tolerance['angular'])
        sdk=record_curve(keep(control))if control is not None else None
        path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
        options=R.FileIO.FileWriteOptions();options.FileVersion=8;options.SuppressDialogBoxes=True;options.SuppressAllInput=True;options.WriteUserData=True;options.UpdateDocumentPath=False
        try:
            if not doc.Write3dmFile(path,options):raise ValueError('source artifact write failed')
        finally:options.Dispose()
        doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros SimplifyCrv '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_SimplifyCrv' if op.get('preselect',True)else '_SimplifyCrv '+' '.join('_SelID '+str(key)for key in source_ids)+' _Enter'
        success,after,events=observe_command(R.Commands.Command,'SimplifyCrv',lambda:R.RhinoApp.RunScript(macro,False),snapshot,lambda:[str(o.Id)for o in doc.Objects if o.IsSelected(False)],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        geometry_changed={o['id']:o['geometry']for o in before}!={o['id']:o['geometry']for o in after}
        undo=redo=None
        if geometry_changed:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,source_ids=[str(k)for k in source_ids],preselect=op.get('preselect',True),before=before,after=after,undo=undo,redo=redo,changed=geometry_changed,sdk=sdk,history=history,events=events),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for index in groups:
            if index>=0 and not doc.Groups.IsDeleted(index):doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for index in layers:doc.Layers.Delete(index,True)
        for a in reversed(attrs):a.Dispose()
        for g in reversed(owned):g.Dispose()
