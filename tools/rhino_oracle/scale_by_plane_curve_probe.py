"""Public Object plane frames across analytic, NURBS, and composite curves."""
import math
import re

if globals().get('__package__'):
    from .number_token import number_token
else:
    from number_token import number_token

TARGETS = ('circle','arc','offset_arc','ellipse',
    'nurbs_circle','nurbs_arc','nurbs_offset_arc','nurbs_ellipse',
    'single_circle','single_arc','single_offset_arc','single_ellipse',
    'poly_lines','poly_nurbs_line','poly_arc_line','poly_nonplanar')


def request():
    return dict(protocol_version=1,iterations=1,operations=[
        dict(op='scale_by_plane_curve',id='plane-curve-'+str(i),target=target,plane=plane,reverse=reverse)
        for i,(target,plane,reverse) in enumerate((target,plane,reverse) for target in TARGETS
            for plane in ('world','tilted') for reverse in (False,True))])


def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=64):
        raise ValueError('ScaleByPlane curve frames require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','target','plane','reverse'}
                or op['op']!='scale_by_plane_curve' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen
                or op['target'] not in TARGETS or op['plane'] not in ('world','tilted')
                or type(op['reverse']) is not bool):
            raise ValueError('invalid ScaleByPlane curve frame recipe')
        seen.add(op['id'])


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System = host['Rhino'],host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('curve frames require idle execution')
    if list(doc.Objects): raise ValueError('curve frames require an empty owned document')
    from join_probe import observe_command
    vp = doc.Views.ActiveView.ActiveViewport; old = vp.GetConstructionPlane()
    x,y = ([1.,1.,0.],[-1.,1.,2.]) if op['plane']=='tilted' else ([.6,.8,0.],[-.8,.6,0.])
    frame = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(*x),Rhino.Geometry.Vector3d(*y))
    def point(coords): return frame.Origin+coords[0]*frame.XAxis+coords[1]*frame.YAxis+coords[2]*frame.ZAxis
    def plane_value(p): return dict(origin=host['_xyz'](p.Origin),x_axis=host['_xyz'](p.XAxis),y_axis=host['_xyz'](p.YAxis))
    def value(curve):
        row = dict(kind=curve.GetType().Name,curve=host['_nurbs_curve_definition'](curve.ToNurbsCurve()))
        if isinstance(curve,Rhino.Geometry.ArcCurve):
            arc = curve.Arc
            row['arc'] = dict(plane=plane_value(arc.Plane),radius=float(arc.Radius),
                angles=[float(arc.AngleDomain.T0),float(arc.AngleDomain.T1)],is_circle=bool(curve.IsCircle()))
        if isinstance(curve,Rhino.Geometry.PolyCurve):
            row['segments'] = [value(curve.SegmentCurve(i)) for i in range(curve.SegmentCount)]
            row['parameters'] = [float(curve.SegmentDomain(i).T0) for i in range(curve.SegmentCount)]+[float(curve.Domain.T1)]
        return row
    ids,target_id = [],None
    target = None
    def snapshot():
        return [dict(role='source' if obj.Id in ids else 'output',name=obj.Attributes.Name,
            selected=bool(obj.IsSelected(False)),point=host['_xyz'](obj.Geometry.Location))
            for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber) if obj.Id!=target_id]
    try:
        kind = op['target']; base = kind
        for prefix in ('nurbs_','single_'):
            if base.startswith(prefix): base = base[len(prefix):]
        if base in ('circle','arc','offset_arc','ellipse'):
            circle = Rhino.Geometry.Circle(frame,2.)
            if base=='circle': target = Rhino.Geometry.ArcCurve(circle)
            elif base=='ellipse': target = Rhino.Geometry.Ellipse(frame,3.,2.).ToNurbsCurve()
            else:
                angles = Rhino.Geometry.Interval(.35,2.2) if base=='offset_arc' else Rhino.Geometry.Interval(0.,math.pi*.75)
                target = Rhino.Geometry.ArcCurve(Rhino.Geometry.Arc(circle,angles))
            if kind.startswith('nurbs_'):
                converted = target.ToNurbsCurve(); target.Dispose(); target = converted
            elif kind.startswith('single_'):
                composite = Rhino.Geometry.PolyCurve()
                if not composite.AppendSegment(target): raise ValueError('single curve append failed')
                target.Dispose(); target = composite
        else:
            target = Rhino.Geometry.PolyCurve()
            if kind in ('poly_lines','poly_nonplanar'):
                vertices = [[0.,0.,0.],[4.,1.,0.],[3.,5.,0.],[-1.,3.,1. if kind=='poly_nonplanar' else 0.]]
                segments = [Rhino.Geometry.LineCurve(point(a),point(b)) for a,b in zip(vertices,vertices[1:])]
            elif kind=='poly_nurbs_line':
                curve = Rhino.Geometry.NurbsCurve(3,True,4,4)
                for i,(coords,weight) in enumerate(zip([[-2.,-1.,0.],[0.,3.,0.],[4.,2.,0.],[3.,-1.,0.]],[1.,2.,.5,1.])):
                    curve.Points.SetPoint(i,point(coords),weight)
                for i,knot in enumerate([0.,0.,0.,1.,1.,1.]): curve.Knots[i] = knot
                segments = [curve,Rhino.Geometry.LineCurve(point([3.,-1.,0.]),point([6.,2.,0.]))]
            else:
                arc = Rhino.Geometry.Arc(Rhino.Geometry.Circle(frame,2.),math.pi*.5)
                segments = [Rhino.Geometry.ArcCurve(arc),Rhino.Geometry.LineCurve(arc.EndPoint,point([-2.,3.,0.]))]
            for segment in segments:
                try:
                    if not target.AppendSegment(segment): raise ValueError('polycurve append failed')
                finally: segment.Dispose()
        if not target.IsValid: raise ValueError('invalid curve frame target')
        if op['reverse'] and not target.Reverse(): raise ValueError('curve frame reversal failed')
        serial = doc.BeginUndoRecord('ScaleByPlane curve frame sources')
        try:
            for i,coords in enumerate([[2.,3.,4.],[2.,2.,3.],[1.,3.,3.],[1.,2.,4.]]):
                attributes = Rhino.DocObjects.ObjectAttributes(); attributes.Name = 'curve-plane source '+str(i)
                source_id = doc.Objects.AddPoint(host['_point'](coords),attributes)
                if source_id==System.Guid.Empty: raise ValueError('curve frame source insertion failed')
                ids.append(source_id)
            target_id = doc.Objects.AddCurve(target)
            if target_id==System.Guid.Empty: raise ValueError('curve frame target insertion failed')
        finally: doc.EndUndoRecord(serial)
        target_value = value(doc.Objects.FindId(target_id).Geometry)
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        cplane_macro = '_CPlane _Object _SelID '+str(target_id)+' _Cancel'
        marker = 'Viboceros curve frame CPlane '+str(System.Guid.NewGuid()); Rhino.RhinoApp.WriteLine(marker)
        cplane_success,cplane,cplane_events = observe_command(Rhino.Commands.Command,'CPlane',
            lambda:Rhino.RhinoApp.RunScript(cplane_macro,True),lambda:plane_value(vp.GetConstructionPlane().Plane),lambda:[],True)
        cplane_history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        for source_id in ids: doc.Objects.Select(source_id)
        origin,reference,destination = [1.,2.,3.],[3.,5.,8.],[5.,11.,1.]
        macro = '_ScaleByPlane _Copy=_No _Rigid=_No _Plane=_Object _SelID '+str(target_id)
        macro += ' '+' '.join('w'+','.join(number_token(float(x)) for x in p) for p in (origin,reference,destination))+' _Cancel'
        before = snapshot(); marker = 'Viboceros curve frame ScaleByPlane '+str(System.Guid.NewGuid()); Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success,after,events = observe_command(Rhino.Commands.Command,'ScaleByPlane',
            lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if Rhino.Commands.Command.InCommand(): raise ValueError('curve frame macro remains active')
        if ([e['result'] for e in events if e['name']=='ScaleByPlane']!=['Success']
                or [e['result'] for e in cplane_events if e['name']=='CPlane']!=['Success']):
            raise ValueError('curve frame public command failed')
        after_script = snapshot(); history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False); undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False); redo = snapshot()
        return dict(target=target_value,target_frame=plane_value(frame),cplane=cplane,
            cplane_macro=cplane_macro,cplane_success=cplane_success,cplane_events=cplane_events,cplane_history=cplane_history,
            origin=origin,reference=reference,destination=destination,macro=macro,events=events,
            before=before,after=after,after_script=after_script,undo=undo,redo=redo,history=history,success=success),0
    finally:
        if target is not None: target.Dispose()
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        vp.SetConstructionPlane(old); doc.Views.Redraw()
