"""Owned public ReducePolyline commands and independent SDK reductions."""
import math
import re

FORMS = ('polyline', 'nurbs', 'weighted_nurbs', 'polycurve', 'nested', 'elevated_nurbs', 'line', 'cubic')


def validate(op):
    required = {'op', 'id', 'points', 'form', 'reduction_tolerance', 'artifact_path'}
    optional = {'copy', 'preselect', 'domain', 'mixed', 'entry', 'benchmark'}
    if (not isinstance(op, dict) or not required <= set(op) or set(op)-required-optional
            or op['op'] != 'reduce_polyline_command' or op['form'] not in FORMS
            or not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z', op['id']) is None
            or not isinstance(op['artifact_path'], str) or not op['artifact_path'].endswith('.3dm')):
        raise ValueError('invalid ReducePolyline recipe')
    if op.get('entry','number') not in ('number','default'):raise ValueError('invalid tolerance entry')
    points = op['points']
    if (not isinstance(points, list) or not 2 <= len(points) <= 2048
            or any(not isinstance(p, list) or len(p) != 3 or any(type(x) not in (int, float)
                   or math.isnan(x) or math.isinf(x) or abs(x) > 1e12 for x in p) for p in points)
            or any(a == b for a, b in zip(points, points[1:]))):
        raise ValueError('invalid bounded source vertices')
    value = op['reduction_tolerance']
    if type(value) not in (int, float) or not 0 < value <= 1e6:
        raise ValueError('invalid reduction tolerance')
    if op['form'] == 'cubic' and len(points) != 4:
        raise ValueError('cubic source requires four controls')
    if op['form'] == 'line' and len(points) != 2:
        raise ValueError('line source requires two endpoints')
    for key in ('copy', 'preselect', 'mixed', 'benchmark'):
        if type(op.get(key, False)) is not bool:
            raise ValueError('invalid ReducePolyline input flag')
    if 'domain' in op:
        d = op['domain']
        if (not isinstance(d, list) or len(d) != 2 or any(type(x) not in (int, float)
                or math.isnan(x) or math.isinf(x) for x in d) or not d[0] < d[1]):
            raise ValueError('invalid source domain')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('ReducePolyline capture requires one iteration')
    R, S = host['Rhino'], host['System']; G = R.Geometry; doc = R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if R.Commands.Command.InCommand() or list(doc.Objects):
        raise ValueError('ReducePolyline requires an owned empty idle document')
    owned = []; attributes = []; layers = []; groups = []
    saved_layer = doc.Layers.CurrentLayerIndex
    def keep(c):
        if c is None or not c.IsValid: raise ValueError('invalid owned ReducePolyline source')
        owned.append(c); return c
    def record(c):
        if not c.IsValid:
            if not isinstance(c,G.PolylineCurve): raise ValueError('unexpected invalid native output')
            return dict(type='invalid_polyline',valid=False,vertices=[host['_xyz'](c.Point(i)) for i in range(c.PointCount)],domain=[float(c.Domain.T0),float(c.Domain.T1)])
        v = host['_interchange_curve_record'](c)
        if isinstance(c, G.NurbsCurve): v['rational'] = bool(c.IsRational)
        if isinstance(c, G.PolylineCurve):
            v['vertices'] = [host['_xyz'](c.Point(i)) for i in range(c.PointCount)]
            v['parameters'] = host['_nurbs_curve_definition'](c)['knots'][1:-1]
        if isinstance(c, G.PolyCurve): v['segments'] = [record(c.SegmentCurve(i)) for i in range(c.SegmentCount)]
        return v
    def strings(o):
        data = o.GetUserStrings()
        return dict((str(k), str(data[k])) for k in data.AllKeys) if data else {}
    def snapshot():
        return [dict(id=str(o.Id), name=str(o.Attributes.Name) if o.Attributes.Name else None,
            layer=int(o.Attributes.LayerIndex), groups=list(int(i) for i in (o.Attributes.GetGroupList() or [])),
            color_source=str(o.Attributes.ColorSource),
            color=[int(o.Attributes.ObjectColor.R), int(o.Attributes.ObjectColor.G), int(o.Attributes.ObjectColor.B)],
            user_text=strings(o.Attributes), geometry_text=strings(o.Geometry),
            selected=bool(o.IsSelected(False)), geometry=record(o.Geometry)) for o in doc.Objects]
    try:
        pts = [G.Point3d(*p) for p in op['points']]; form = op['form']; n = len(pts)
        if form == 'polyline': source = keep(G.PolylineCurve(S.Array[G.Point3d](pts)))
        elif form == 'line': source = keep(G.LineCurve(pts[0], pts[1]))
        elif form in ('nurbs', 'weighted_nurbs', 'elevated_nurbs', 'cubic'):
            degree = 3 if form == 'cubic' else 1
            knots = [0.]*4+[1.]*4 if degree == 3 else [0.]+[float(i) for i in range(n)]+[float(n-1)]
            source = keep(host['_nurbs_curve_from_definition'](dict(degree=degree, knots=knots,
                control_points=[dict(point=p, weight=1.+i%3 if form == 'weighted_nurbs' else 1.) for i,p in enumerate(op['points'])])))
            if form != 'weighted_nurbs': source.Points.MakeNonRational()
            if form == 'elevated_nurbs' and not source.IncreaseDegree(3):raise ValueError('native degree elevation failed')
        else:
            parts = [keep(G.LineCurve(a,b)) for a,b in zip(pts,pts[1:])]
            source = G.PolyCurve(); owned.append(source)
            if form == 'nested' and len(parts) > 2:
                child = G.PolyCurve(); owned.append(child)
                for part in parts[:2]:
                    if not child.Append(part): raise ValueError('child append failed')
                parts = [child]+parts[2:]
            for part in parts:
                if not source.Append(part): raise ValueError('source append failed')
        if 'domain' in op: source.Domain = G.Interval(*op['domain'])
        source.SetUserString('GeometryCode','source')
        success, pl = source.TryGetPolyline()
        sdk = None
        if success:
            removed = pl.ReduceSegments(op['reduction_tolerance'])
            sdk = dict(removed=int(removed), points=[host['_xyz'](p) for p in pl])
            if op.get('benchmark'):
                durations=[]
                for index in range(5):
                    trial=G.Polyline(pts);timer=S.Diagnostics.Stopwatch.StartNew()
                    trial.ReduceSegments(op['reduction_tolerance']);timer.Stop()
                    if index>=2:durations.append(int(float(timer.ElapsedTicks)*1e9/float(S.Diagnostics.Stopwatch.Frequency)))
                sdk['elapsed_ns']=durations
        layer = R.DocObjects.Layer(); layer.Name = 'ReduceSource_'+str(S.Guid.NewGuid())
        try: layer_index = doc.Layers.Add(layer)
        finally: layer.Dispose()
        layers.append(layer_index)
        sources = [source]
        if op.get('mixed'):
            witness = keep(G.PolylineCurve(S.Array[G.Point3d]([G.Point3d(20.,0.,0.),G.Point3d(21.,.01,0.),G.Point3d(22.,0.,0.),G.Point3d(23.,2.,0.)])))
            witness.SetUserString('GeometryCode','source'); sources.append(witness)
        ids = []
        for i,c in enumerate(sources):
            a = R.DocObjects.ObjectAttributes(); attributes.append(a); a.Name = 'ReduceSource'+str(i)
            a.LayerIndex = layer_index; a.SetUserString('ObjectCode','source')
            a.ObjectColor = S.Drawing.Color.FromArgb(17,47,87); a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
            key = doc.Objects.AddCurve(c,a)
            if key == S.Guid.Empty: raise ValueError('source insertion failed')
            ids.append(key)
        group = doc.Groups.Add('ReduceGroup_'+str(S.Guid.NewGuid()),S.Array[S.Guid](ids)); groups.append(group)
        if op.get('preselect', True):
            for key in ids: doc.Objects.Select(key)
        path=op['artifact_path']; path='Z:'+path.replace('/','\\') if path.startswith('/') else path
        options=R.FileIO.FileWriteOptions(); options.FileVersion=8; options.SuppressDialogBoxes=True
        options.SuppressAllInput=True; options.WriteUserData=True; options.UpdateDocumentPath=False
        try:
            if not doc.Write3dmFile(path,options): raise ValueError('source artifact write failed')
        finally: options.Dispose()
        doc.ClearUndoRecords(True); before=snapshot()
        marker='Viboceros ReducePolyline '+str(S.Guid.NewGuid()); R.RhinoApp.WriteLine(marker)
        macro='_ReducePolyline '
        if not op.get('preselect',True): macro+=' '.join('_SelID '+str(k) for k in ids)+' _Enter '
        macro+='_Copy=_%s ' % ('Yes' if op.get('copy',False) else 'No')
        if op.get('entry','number')=='number':macro+='%.17g'%op['reduction_tolerance']
        else:macro+='_Enter'
        success, after, events = observe_command(R.Commands.Command,'ReducePolyline',
            lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[str(o.Id) for o in doc.Objects if o.IsSelected(False)],True)
        changed=[(o['id'],o['geometry']) for o in before]!=[(o['id'],o['geometry']) for o in after]
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]; undo=redo=None
        if changed:
            R.RhinoApp.RunScript('_Undo',False); undo=snapshot(); R.RhinoApp.RunScript('_Redo',False); redo=snapshot()
        return dict(source_ids=[str(k) for k in ids],recipe=op,success=success,before=before,after=after,
            changed=changed,undo=undo,redo=redo,sdk=sdk,macro=macro,history=history,events=events,document_tolerance=float(doc.ModelAbsoluteTolerance)),0
    finally:
        if R.Commands.Command.InCommand(): R.RhinoApp.RunScript('!',False)
        for o in list(doc.Objects): doc.Objects.Delete(o.Id,True)
        for i in groups:
            if i>=0 and not doc.Groups.IsDeleted(i): doc.Groups.Delete(i)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for i in layers: doc.Layers.Delete(i,True)
        for a in reversed(attributes): a.Dispose()
        for c in reversed(owned): c.Dispose()
