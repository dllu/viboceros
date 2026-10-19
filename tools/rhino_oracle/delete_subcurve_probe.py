"""Public DeleteSubCrv on exact owned curve inputs with retained history."""
import math
import re
FAMILIES = ('line','arc','circle','nurbs','nurbs_circle','polyline','closed_polyline','polycurve','ellipse')

def validate(op):
    required={'op','id','family','start','end','artifact_path'}
    optional={'length','length_fraction','preselect','domain','reverse','confirmation'}
    finite=lambda x:type(x)in(int,float) and not math.isnan(x) and not math.isinf(x)
    if (not isinstance(op,dict) or not required<=set(op) or set(op)-required-optional
        or op['op']!='delete_subcurve_command' or op['family']not in FAMILIES
        or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id'])is None
        or not isinstance(op['artifact_path'],str) or not op['artifact_path'].endswith('.3dm')
        or not finite(op['start']) or not 0<=op['start']<=1
        or not finite(op['end']) or not 0<=op['end']<=1):
        raise ValueError('invalid DeleteSubCrv recipe')
    if 'length'in op and (not finite(op['length']) or abs(op['length'])>100):raise ValueError('invalid bounded deletion length')
    if 'length_fraction'in op and ('length'in op or not finite(op['length_fraction']) or not 0<op['length_fraction']<=2):raise ValueError('invalid relative deletion length')
    for key in('preselect','reverse','confirmation'):
        if type(op.get(key,False))is not bool:raise ValueError('invalid DeleteSubCrv flag')
    if 'domain'in op:
        d=op['domain']
        if not isinstance(d,list)or len(d)!=2 or not all(finite(x)for x in d)or not d[0]<d[1]:raise ValueError('invalid source domain')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('DeleteSubCrv requires one iteration')
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if R.Commands.Command.InCommand()or list(doc.Objects):raise ValueError('DeleteSubCrv requires an empty owned idle document')
    owned=[];attributes=[];layers=[];groups=[];saved_layer=doc.Layers.CurrentLayerIndex
    saved_tolerance=(doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
    def keep(c):
        if c is None or not c.IsValid:raise ValueError('invalid source curve')
        owned.append(c);return c
    def record(c):
        v=host['_interchange_curve_record'](c)
        if isinstance(c,G.NurbsCurve):v['rational']=bool(c.IsRational)
        if isinstance(c,G.ArcCurve):
            a=c.Arc;v['arc']=dict(center=host['_xyz'](a.Center),x=host['_xyz'](a.Plane.XAxis),normal=host['_xyz'](a.Plane.ZAxis),radius=float(a.Radius),angles=[float(a.StartAngle),float(a.EndAngle)])
        if isinstance(c,G.PolylineCurve):
            v['vertices']=[host['_xyz'](c.Point(i))for i in range(c.PointCount)]
            n=c.ToNurbsCurve()
            try:v['parameters']=host['_nurbs_curve_definition'](n)['knots'][1:-1]
            finally:n.Dispose()
        if isinstance(c,G.PolyCurve):v['segments']=[record(c.SegmentCurve(i))for i in range(c.SegmentCount)]
        return v
    def strings(o):
        d=o.GetUserStrings();return dict((str(k),str(d[k]))for k in d.AllKeys)if d else{}
    def snapshot():
        return [dict(id=str(o.Id),name=o.Attributes.Name or None,layer=int(o.Attributes.LayerIndex),groups=list(int(i)for i in(o.Attributes.GetGroupList()or[])),user_text=strings(o.Attributes),geometry_text=strings(o.Geometry),color_source=str(o.Attributes.ColorSource),color=[int(o.Attributes.ObjectColor.R),int(o.Attributes.ObjectColor.G),int(o.Attributes.ObjectColor.B)],selected=bool(o.IsSelected(False)),geometry=record(o.Geometry))for o in doc.Objects]
    try:
        family=op['family']
        if family=='line':source=keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(10.,0.,0.)))
        elif family=='nurbs':source=keep(host['_nurbs_curve_from_definition'](dict(degree=3,knots=[0.]*4+[1.]*4,control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[2.,5.,1.],[7.,-2.,2.],[10.,0.,0.]]])))
        elif family in('arc','circle','nurbs_circle'):
            source=keep(G.ArcCurve(G.Arc(G.Circle(G.Plane.WorldXY,3.),G.Interval(0.,2.*math.pi if family!='arc'else .75*math.pi))))
            if family=='nurbs_circle':source=keep(source.ToNurbsCurve())
        elif family=='ellipse':source=keep(G.Ellipse(G.Plane.WorldXY,4.,2.).ToNurbsCurve())
        elif family in('polyline','closed_polyline'):
            pts=[[0.,0.,0.],[4.,0.,0.],[4.,6.,0.],[0.,6.,0.]]
            if family=='closed_polyline':pts.append(pts[0])
            source=keep(G.PolylineCurve(S.Array[G.Point3d]([G.Point3d(*p)for p in pts])))
        else:
            a=keep(G.LineCurve(G.Point3d(3.,-4.,0.),G.Point3d(3.,0.,0.)))
            b=keep(G.ArcCurve(G.Arc(G.Circle(G.Plane.WorldXY,3.),G.Interval(0.,math.pi*.5))))
            c=keep(G.LineCurve(G.Point3d(0.,3.,0.),G.Point3d(-4.,3.,0.)))
            source=G.PolyCurve();owned.append(source)
            for part in(a,b,c):
                if not source.Append(part):raise ValueError('polycurve append failed')
        if op.get('reverse'):source.Reverse()
        if 'domain'in op:source.Domain=G.Interval(*op['domain'])
        source.SetUserString('GeometryCode','source')
        for suffix in('input','output'):
            layer=R.DocObjects.Layer();layer.Name='DeleteSubCrv_'+suffix+'_'+str(S.Guid.NewGuid())
            try:layers.append(doc.Layers.Add(layer))
            finally:layer.Dispose()
            if layers[-1]<0:raise ValueError('source layer creation failed')
        a=R.DocObjects.ObjectAttributes();attributes.append(a);a.Name='DeleteSource';a.LayerIndex=layers[0];a.SetUserString('ObjectCode','source');a.ObjectColor=S.Drawing.Color.FromArgb(17,47,87);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        key=doc.Objects.AddCurve(source,a)
        if key==S.Guid.Empty:raise ValueError('source insertion failed')
        group=doc.Groups.Add('DeleteGroup_'+str(S.Guid.NewGuid()),S.Array[S.Guid]([key]));groups.append(group)
        if group<0:raise ValueError('source group failed')
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        if op.get('preselect',True):doc.Objects.Select(key)
        start=source.Domain.ParameterAt(op['start']);end=source.Domain.ParameterAt(op['end'])
        start_point=host['_xyz'](source.PointAt(start));end_point=host['_xyz'](source.PointAt(end))
        path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
        options=R.FileIO.FileWriteOptions();options.FileVersion=8;options.SuppressDialogBoxes=True;options.SuppressAllInput=True;options.WriteUserData=True;options.UpdateDocumentPath=False
        try:
            if not doc.Write3dmFile(path,options):raise ValueError('source write failed')
        finally:options.Dispose()
        doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros DeleteSubCrv '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        point=lambda p:','.join(repr(v)for v in p)
        macro='_DeleteSubCrv '
        if not op.get('preselect',True):macro+='_SelID '+str(key)+' '
        macro+=point(start_point)+' '
        length=op.get('length',source.GetLength()*op['length_fraction'] if 'length_fraction'in op else None)
        if length is not None:macro+=repr(length)+' '
        if op.get('confirmation',True):macro+=point(end_point)+' '
        macro+='_Enter'
        success,after,events=observe_command(R.Commands.Command,'DeleteSubCrv',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[str(o.Id)for o in doc.Objects if o.IsSelected(False)],True)
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        changed={o['id']:o['geometry']for o in before}!={o['id']:o['geometry']for o in after}
        undo=redo=None
        if changed:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,command_result=[e['result']for e in events if e['name']=='DeleteSubCrv'][0],source_id=str(key),start_parameter=start,end_parameter=end,start_point=start_point,end_point=end_point,before=before,after=after,undo=undo,redo=redo,changed=changed,macro=macro,history=history,events=events,recipe=op,length=length,document_tolerance=dict(absolute=doc.ModelAbsoluteTolerance,relative=doc.ModelRelativeTolerance,angular=doc.ModelAngleToleranceRadians)),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for index in groups:
            if index>=0 and not doc.Groups.IsDeleted(index):doc.Groups.Delete(index)
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=saved_tolerance
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        for a in reversed(attributes):a.Dispose()
        for c in reversed(owned):c.Dispose()
