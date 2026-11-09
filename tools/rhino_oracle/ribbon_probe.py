"""Owned public Ribbon commands with source, output and history readback."""
import math

def finite(x):
    return not math.isnan(x) and not math.isinf(x)


def validate(op):
    required = {'op','id','curve','distance','side','both_sides','corner','loose','in_cplane','output_layer'}
    if not isinstance(op,dict) or not required.issubset(op) or set(op)-required-{'native_kind','postselected','fit_tolerance','cplane','model_angle_tolerance'} or op['op'] not in ['ribbon_command','loose_offset_command']:
        raise ValueError('invalid Ribbon operation')
    if not isinstance(op['curve'],dict) or not isinstance(op['id'],str):
        raise ValueError('invalid Ribbon curve or id')
    if op.get('native_kind','nurbs') not in ['nurbs','line','circle','polyline','polycurve','periodic']:
        raise ValueError('invalid native curve kind')
    if any(type(op[k]) is not bool for k in ['both_sides','loose','in_cplane']):
        raise ValueError('invalid Ribbon switch')
    if type(op.get('postselected',False)) is not bool:
        raise ValueError('invalid Ribbon selection mode')
    if 'fit_tolerance'in op and (type(op['fit_tolerance'])not in(int,float)or not finite(op['fit_tolerance'])or op['fit_tolerance']<0. or op['loose']):
        raise ValueError('invalid Ribbon fit tolerance')
    if 'model_angle_tolerance'in op and (type(op['model_angle_tolerance'])not in(int,float)or not finite(op['model_angle_tolerance'])or not 0.<op['model_angle_tolerance']<=math.pi):
        raise ValueError('invalid Ribbon model angle tolerance')
    if 'cplane'in op:
        frame=op['cplane']
        if not isinstance(frame,list)or len(frame)!=3 or any(not isinstance(v,list)or len(v)!=3 or any(type(x)not in(int,float)or not finite(x)for x in v)for v in frame):
            raise ValueError('invalid Ribbon construction plane')
        x,y=frame[1:];cross=[x[1]*y[2]-x[2]*y[1],x[2]*y[0]-x[0]*y[2],x[0]*y[1]-x[1]*y[0]]
        if not any(cross)or any(not finite(v)for v in cross):raise ValueError('degenerate Ribbon construction plane')
    if op['corner'] not in ['Sharp','Round','Smooth','Chamfer','None'] or op['output_layer'] not in ['Current','Input']:
        raise ValueError('invalid Ribbon choice')
    if type(op['distance']) not in (int,float) or not finite(op['distance']) or op['distance'] <= 0.:
        raise ValueError('invalid Ribbon distance')
    if not isinstance(op['side'],list) or len(op['side']) != 3 or any(type(x) not in (int,float) or not finite(x) for x in op['side']):
        raise ValueError('invalid Ribbon side')


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command,record_brep
    R,S = host['Rhino'],host['System']; G = R.Geometry; doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('Ribbon requires an owned empty idle document')
    viewport=doc.Views.ActiveView.ActiveViewport
    old = doc.Layers.CurrentLayerIndex,doc.ModelAbsoluteTolerance,viewport.GetConstructionPlane(),doc.ModelAngleToleranceRadians
    layers=[]; group=None; source=None
    def snapshot():
        records=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            a,g=obj.Attributes,obj.Geometry
            row=dict(source=obj.Id==source,selected=bool(obj.IsSelected(False)),native_type=g.GetType().Name,
                     valid=bool(g.IsValid),attributes=dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,
                         color_source=str(a.ColorSource),color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                         groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])]))
            if isinstance(g,G.Curve): row['curve']=host['_nurbs_curve_definition'](g)
            elif isinstance(g,G.Brep):
                row['surfaces']=[host['_nurbs_surface_definition'](f.UnderlyingSurface()) for f in g.Faces]
                row['brep']=record_brep(g,tolerance,host,True)
            else: raise ValueError('unexpected Ribbon geometry')
            records.append(row)
        return records
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute']
        if 'model_angle_tolerance'in op:doc.ModelAngleToleranceRadians=op['model_angle_tolerance']
        if 'cplane'in op:
            origin,x,y=op['cplane'];viewport.SetConstructionPlane(G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y)))
        for name in ['Ribbon source','Ribbon output']:
            layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer));layer.Dispose()
        curve=host['_nurbs_curve_from_definition'](op['curve'])
        kind=op.get('native_kind','nurbs')
        if kind!='nurbs':
            original=curve
            try:
                points=[host['_point'](p['point']) for p in op['curve']['control_points']]
                if kind=='line':curve=G.LineCurve(points[0],points[-1])
                elif kind=='polyline':curve=G.PolylineCurve(points)
                elif kind=='polycurve':
                    curve=G.PolyCurve()
                    for a,b in zip(points,points[1:]):
                        line=G.LineCurve(a,b)
                        try:curve.Append(line)
                        finally:line.Dispose()
                elif kind=='circle':
                    ok,circle=original.TryGetCircle(tolerance['absolute'])
                    if not ok:raise ValueError('source is not a circle')
                    curve=G.ArcCurve(circle)
                else:curve=G.NurbsCurve.Create(True,op['curve']['degree'],points)
                if curve is None or not curve.IsValid:raise ValueError('invalid native curve variant')
            finally:original.Dispose()
        projection=curve.DuplicateCurve()
        try:
            projection.Transform(G.Transform.PlanarProjection(viewport.ConstructionPlane()))
            ok,plane=curve.TryGetPlane(tolerance['absolute'])
            source_analysis=dict(planar=bool(ok),plane_normal=[plane.ZAxis.X,plane.ZAxis.Y,plane.ZAxis.Z]if ok else None,
                projected_linear=bool(projection.IsLinear(tolerance['absolute'])),
                model_angle_tolerance=doc.ModelAngleToleranceRadians)
            points=[host['_point'](p['point'])for p in op['curve']['control_points']]
            source_analysis['fit_plane_normal']=None
            if len(points)>=3:
                result,fitted=G.Plane.FitPlaneToPoints(points)
                if fitted.IsValid:
                    source_analysis['fit_plane_normal']=[fitted.ZAxis.X,fitted.ZAxis.Y,fitted.ZAxis.Z]
        finally:projection.Dispose()
        attributes=R.DocObjects.ObjectAttributes();attributes.Name='Ribbon guide';attributes.LayerIndex=layers[0]
        attributes.ObjectColor=S.Drawing.Color.FromArgb(12,34,56);attributes.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        try: source=doc.Objects.AddCurve(curve,attributes)
        finally:curve.Dispose();attributes.Dispose()
        group=doc.Groups.Add('Ribbon group',[source])
        if not op.get('postselected',False):doc.Objects.Select(source)
        doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.ClearUndoRecords(True)
        before=snapshot()
        words=['_Distance','%.17g'%op['distance'],'_Loose=_'+('Yes' if op['loose'] else 'No')]
        if not op['loose']:words+=['_Corner=_'+op['corner']]
        if 'fit_tolerance'in op:words+=['_Tolerance','%.17g'%op['fit_tolerance']]
        words+=['_InCPlane=_'+('Yes' if op['in_cplane'] else 'No'),'_OutputLayer=_'+op['output_layer']]
        if op['both_sides']:words.append('_BothSides')
        words.append('w'+','.join('%.17g'%x for x in op['side']))
        command='Ribbon' if op['op']=='ribbon_command' else 'Offset'
        selection=('_SelID '+str(source)+' ') if op.get('postselected',False) else ''
        macro='_'+command+' '+selection+' '.join(words)
        marker='Owned Ribbon '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,created,events=observe_command(R.Commands.Command,command,
            lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand():raise ValueError('Ribbon did not finish: '+history)
        after=snapshot()
        if success:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
            R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:undo=before;redo=after
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,
                    history=history,macro=macro,events=events,returned_idle=True,source_analysis=source_analysis),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        if group is not None:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old[0],True);doc.ModelAbsoluteTolerance=old[1]
        viewport.SetConstructionPlane(old[2])
        doc.ModelAngleToleranceRadians=old[3]
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
