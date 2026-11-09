"""Owned public Ribbon commands with source, output and history readback."""
import math

def finite(x):
    return not math.isnan(x) and not math.isinf(x)


def validate(op):
    required = {'op','id','curve','distance','side','both_sides','corner','loose','in_cplane','output_layer'}
    if not isinstance(op,dict) or not required.issubset(op) or set(op)-required-{'native_kind'} or op['op'] not in ['ribbon_command','loose_offset_command']:
        raise ValueError('invalid Ribbon operation')
    if not isinstance(op['curve'],dict) or not isinstance(op['id'],str):
        raise ValueError('invalid Ribbon curve or id')
    if op.get('native_kind','nurbs') not in ['nurbs','line','circle','polyline','polycurve','periodic']:
        raise ValueError('invalid native curve kind')
    if any(type(op[k]) is not bool for k in ['both_sides','loose','in_cplane']):
        raise ValueError('invalid Ribbon switch')
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
    old = doc.Layers.CurrentLayerIndex,doc.ModelAbsoluteTolerance
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
        attributes=R.DocObjects.ObjectAttributes();attributes.Name='Ribbon guide';attributes.LayerIndex=layers[0]
        attributes.ObjectColor=S.Drawing.Color.FromArgb(12,34,56);attributes.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        try: source=doc.Objects.AddCurve(curve,attributes)
        finally:curve.Dispose();attributes.Dispose()
        group=doc.Groups.Add('Ribbon group',[source]);doc.Objects.Select(source)
        doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.ClearUndoRecords(True)
        before=snapshot()
        words=['_Distance','%.17g'%op['distance'],'_Loose=_'+('Yes' if op['loose'] else 'No')]
        if not op['loose']:words+=['_Corner=_'+op['corner']]
        words+=['_InCPlane=_'+('Yes' if op['in_cplane'] else 'No'),'_OutputLayer=_'+op['output_layer']]
        if op['both_sides']:words.append('_BothSides')
        words.append('w'+','.join('%.17g'%x for x in op['side']))
        command='Ribbon' if op['op']=='ribbon_command' else 'Offset'
        macro='_'+command+' '+' '.join(words)
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
                    history=history,macro=macro,events=events,returned_idle=True),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        if group is not None:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old[0],True);doc.ModelAbsoluteTolerance=old[1]
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
