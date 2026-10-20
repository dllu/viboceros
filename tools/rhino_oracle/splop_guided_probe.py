"""Owned Splop reference-sphere fitting and curve-guided public inputs."""
import math,re
try:
    from . import splop_probe as base
except (ImportError,ValueError):
    import splop_probe as base
FIT_POINTS={
 'Tetra':[[1.,1.,1.],[1.,-1.,-1.],[-1.,1.,-1.],[-1.,-1.,1.]],
 'Axes':[[4.,2.,3.],[-2.,2.,3.],[1.,5.,3.],[1.,-1.,3.],[1.,2.,6.],[1.,2.,0.]],
 'Planar':[[2.,0.,1.],[0.,2.,1.],[-2.,0.,1.],[0.,-2.,1.]],
 'Noisy':[[4.03,2.,3.],[-2.,2.02,3.],[1.,5.,3.01],[1.,-1.04,3.],[1.,2.,6.02],[1.,2.,-.01]],
}
CURVES=('LineZ','LineX','LineSkew','Circle','Cubic')
SIZES=('Radius','Diameter','Circumference','Area','Point')
def validate(op):
    required={'op','id','mode','kind','plane'}
    optional={'source','size','surface','copy','rigid','flip','finish'}
    if not isinstance(op,dict) or set(op)-optional!=required or op['op']!='splop_guided':raise ValueError('invalid guided sphere recipe')
    if (not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or op['plane']not in('WorldXY','WorldZX','WorldYZ') or
        op['mode']not in('Fit','Around','FourRadius')or op.get('surface','Plane')not in('Plane','Cylinder')or
        any(type(op.get(k,False))is not bool for k in('copy','rigid','flip'))or op.get('finish','Complete')not in('Complete','CancelReference')):raise ValueError('invalid guided sphere controls')
    if op['mode']=='Fit':
        if op['kind']not in FIT_POINTS or op.get('source','Points')not in('Points','CurveGrips','MeshGrips'):raise ValueError('invalid fit guide')
    elif op['mode']=='Around':
        if op['kind']not in CURVES or op.get('size','Radius')not in SIZES:raise ValueError('invalid curve guide')
    elif op['kind']not in('FourthAbove','FourthBelow','Third'):raise ValueError('invalid four-point radius guide')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one guided sphere command required')
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('guided spheres need an empty owned document at idle')
    vp=doc.Views.ActiveView.ActiveViewport;old_plane=vp.GetConstructionPlane();old_tolerance=doc.ModelAbsoluteTolerance
    plane=getattr(G.Plane,op['plane']);vp.SetConstructionPlane(plane);doc.ModelAbsoluteTolerance=1e-6
    surface=G.PlaneSurface(G.Plane(G.Point3d(0.,0.,5.),G.Vector3d.ZAxis),G.Interval(-20.,20.),G.Interval(-20.,20.)).ToNurbsSurface()if op.get('surface','Plane')=='Plane'else G.Cylinder(G.Circle(G.Plane.WorldXY,20.),20.).ToNurbsSurface()
    uv=[surface.Domain(0).ParameterAt(.17),surface.Domain(1).ParameterAt(.61)];ok,frame=surface.FrameAt(*uv)
    if not ok:raise ValueError('unavailable drop frame')
    source=target=None;guides=[];owned=[];reference={};curve=None;parameter=None
    point=lambda p:'w'+','.join('%.17g'%x for x in host['_xyz'](p))
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            if o.Id==target:continue
            g=o.Geometry;row=dict(role='source'if o.Id==source else'guide'if o.Id in guides else'output',guide=guides.index(o.Id)if o.Id in guides else None,
                selected=bool(o.IsSelected(False)),name=o.Attributes.Name,grips_on=bool(o.GripsOn),grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False)))for p in(o.GetGrips()or[])])
            if isinstance(g,G.PointCloud):row['geometry']=dict(kind='PointCloud',points=[host['_xyz'](p.Location)for p in g])
            elif isinstance(g,G.Point):row['geometry']=dict(kind='Point',point=host['_xyz'](g.Location))
            elif isinstance(g,G.Curve):row['geometry']=dict(kind=g.GetType().Name,curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g,G.Mesh):row['geometry']=dict(kind='Mesh',mesh=host['_polygon_mesh_value'](g))
            elif isinstance(g,G.Brep):row['geometry']=dict(kind='Brep',surface=host['_nurbs_surface_definition'](g.Surfaces[0]))
            else:raise ValueError('unsupported guide geometry')
            rows.append(row)
        return dict(objects=rows,target_selected=bool(doc.Objects.FindId(target).IsSelected(False)))
    try:
        record=doc.BeginUndoRecord('Owned Splop guided sources')
        try:
            cloud=G.PointCloud([G.Point3d(*p)for p in[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[2.,1.,.5]]]);owned.append(cloud)
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='Splop source';source=doc.Objects.AddPointCloud(cloud,attributes)
            target=doc.Objects.AddSurface(surface)
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='Reference guide'
            if op['mode']=='Fit':
                points=[G.Point3d(*p)for p in FIT_POINTS[op['kind']]];reference['points']=[host['_xyz'](p)for p in points]
                kind=op.get('source','Points')
                if kind=='Points':guides=[doc.Objects.AddPoint(p,attributes)for p in points]
                elif kind=='CurveGrips':
                    curve=G.NurbsCurve(3,False,2,len(points));owned.append(curve)
                    for i,p in enumerate(points):curve.Points.SetPoint(i,p)
                    curve.Knots.CreateUniformKnots(1.);guides=[doc.Objects.AddCurve(curve,attributes)]
                else:
                    mesh=G.Mesh();owned.append(mesh);mesh.Vertices.UseDoublePrecisionVertices=True
                    for p in points:mesh.Vertices.Add(p)
                    mesh.Faces.AddFace(0,1,2);guides=[doc.Objects.AddMesh(mesh,attributes)]
            elif op['mode']=='Around':
                kind=op['kind']
                if kind.startswith('Line'):
                    a,b={'LineZ':([1.,2.,-4.],[1.,2.,8.]),'LineX':([-4.,2.,1.],[8.,2.,1.]),'LineSkew':([-2.,-1.,-3.],[4.,7.,9.])}[kind]
                    curve=G.LineCurve(G.Point3d(*a),G.Point3d(*b));guides=[doc.Objects.AddCurve(curve,attributes)]
                elif kind=='Circle':
                    curve=G.Circle(G.Plane.WorldYZ,4.).ToNurbsCurve();guides=[doc.Objects.AddCurve(curve,attributes)]
                elif kind=='Cubic':
                    curve=G.NurbsCurve.Create(False,3,[G.Point3d(*p)for p in[[-3.,0.,-1.],[-1.,4.,2.],[2.,-2.,5.],[5.,3.,7.]]]);guides=[doc.Objects.AddCurve(curve,attributes)]
                else:
                    brep=G.PlaneSurface(G.Plane(G.Point3d(1.,2.,3.),G.Vector3d(1.,2.,3.)),G.Interval(-3.,3.),G.Interval(-2.,2.)).ToBrep();owned.append(brep)
                    guides=[doc.Objects.AddBrep(brep,attributes)];curve=brep.Edges[0].DuplicateCurve();reference['edge']=0
                owned.append(curve);parameter=curve.Domain.ParameterAt(.37);center=curve.PointAt(parameter);tangent=curve.TangentAt(parameter);reference.update(curve=host['_nurbs_curve_definition'](curve),center=host['_xyz'](center),parameter=float(parameter),tangent=host['_xyz'](tangent))
        finally:doc.EndUndoRecord(record)
        if op['mode']=='Fit'and op.get('source','Points')!='Points':doc.Objects.FindId(guides[0]).GripsOn=True
        doc.Objects.Select(source);doc.ClearUndoRecords(True);before=snapshot()
        if op['mode']=='Fit':
            selection=' '.join('_SelID '+str(i)for i in guides)if op.get('source','Points')=='Points'else'_SelPt'
            tokens='_FitPoints '+selection+' _Enter'
        elif op['mode']=='Around':
            pick=reference['center'];pick=G.Point3d(*pick)
            tokens='_AroundCurve '
            if op['kind']=='Edge':tokens+=point(pick)+' '
            else:tokens+="'_-SelID "+str(guides[0])+' '
            tokens+=point(pick)+' '
            size=op.get('size','Radius')
            if size=='Point':
                n=G.Plane(pick,curve.TangentAt(parameter));p=pick+n.XAxis*2.+n.YAxis*1.+n.ZAxis*3.;reference['size_point']=host['_xyz'](p);tokens+=point(p)
            else:
                value={'Radius':2.,'Diameter':4.,'Circumference':4.*math.pi,'Area':4.*math.pi}[size]
                tokens+=('_'+size+' 'if size!='Radius'else'')+repr(value)
        else:
            tokens={'FourthAbove':'_4Point w2,0,0 w0,2,0 w-2,0,0 _Radius 3 w0,0,4','FourthBelow':'_4Point w2,0,0 w0,2,0 w-2,0,0 _Radius 3 w0,0,-4','Third':'_4Point w-1,0,0 w1,0,0 _Radius 2 w0,1,0'}[op['kind']]
        flags=' '.join('_'+k+'=_'+('Yes'if op.get(k.lower(),False)else'No')for k in('Copy','Rigid','Flip'))
        macro='_Splop '+tokens
        if op.get('finish')=='CancelReference':macro+=' _Cancel'
        else:
            macro+=" '_-SelID "+str(target)+' '+flags+' '+point(frame.Origin)+' '+point(frame.Origin+frame.XAxis*4.)
            if op.get('copy'):macro+=' _Enter'
        marker='Owned Splop guided '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(R.Commands.Command,'Splop',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot();undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()[len(history):]
        return dict(success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history,undo_history=undo_history,macro=macro,reference=reference,surface=host['_nurbs_surface_definition'](surface),uv=uv,frame=dict(origin=host['_xyz'](frame.Origin),x=host['_xyz'](frame.XAxis),y=host['_xyz'](frame.YAxis))),0
    finally:
        for o in list(doc.Objects):
            if o.GripsOn:o.GripsOn=False
            doc.Objects.Delete(o.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted:doc.Groups.Delete(g.Index)
        for g in owned:g.Dispose()
        surface.Dispose();vp.SetConstructionPlane(old_plane);doc.ModelAbsoluteTolerance=old_tolerance
