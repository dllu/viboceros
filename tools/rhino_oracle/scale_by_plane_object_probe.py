"""Public ScaleByPlane Object targets and separate CPlane Object witnesses."""
import json
import math
import os
import time
import re

if globals().get('__package__'):
    from .number_token import number_token
else:
    from number_token import number_token

TARGETS = ('line', 'polyline', 'rational', 'circle', 'arc', 'ellipse',
           'surface', 'skew_surface', 'mesh', 'warped_surface', 'nonplanar_curve', 'point')


def request():
    operations = [
        dict(op='scale_by_plane_object', id='plane-object-'+str(i), target=target,
             plane=plane, reverse=reverse, copy=False, cplane='world')
        for i,(target,plane,reverse) in enumerate(
            (target,plane,reverse) for target in TARGETS
            for plane in ('world','tilted') for reverse in (False,True))]
    for target in ('polyline','warped_surface','nonplanar_curve','point'):
        for reverse in (False,True):
            operations.append(dict(op='scale_by_plane_object',id='plane-object-'+str(len(operations)),
                target=target,plane='tilted',reverse=reverse,copy=True,cplane='world'))
    for target in ('point','mesh'):
        for plane in ('world','tilted'):
            for reverse in (False,True):
                operations.append(dict(op='scale_by_plane_object',id='plane-object-'+str(len(operations)),
                    target=target,plane=plane,reverse=reverse,copy=False,cplane='tilted'))
    return dict(protocol_version=1,iterations=1,operations=operations)


def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('ScaleByPlane Object requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','target','plane','reverse','copy','cplane'}
                or op['op']!='scale_by_plane_object' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen
                or op['target'] not in TARGETS or op['plane'] not in ('world','tilted')
                or op['cplane'] not in ('world','tilted')
                or type(op['reverse']) is not bool or type(op['copy']) is not bool):
            raise ValueError('invalid ScaleByPlane Object recipe')
        seen.add(op['id'])


def token(point):
    return 'w'+','.join(number_token(float(x)) for x in point)


def cancel_mesh(op, macro, host, delivered):
    """Finish the outer getter after the macro cancels rejected Object input."""
    Rhino = host['Rhino']
    start_history = Rhino.RhinoApp.CommandHistoryWindowText
    requested = []
    marker = '@scale-object-cancel:'+op['id']+':0'
    def ended(sender,event):
        if event.CommandEnglishName=='SelID' and not requested:
            history = Rhino.RhinoApp.CommandHistoryWindowText[len(start_history):]
            if 'No objects added to selection.' in history:
                requested.append(marker); host['_record_progress']('PICK '+marker+' 1 1')
    event = Rhino.Commands.Command.EndCommand
    event += ended
    try:
        result = Rhino.RhinoApp.RunScript(macro,True)
        if requested!=[marker]: raise ValueError('missing public rejected SelID completion')
        # The owned driver writes its receipt after xdotool returns. Wait before
        # another native command, so pending key release cannot reach it.
        acknowledgement = os.path.join(os.path.dirname(os.path.abspath(host['__file__'])),'click-ack.json')
        deadline = time.time()+5.
        while time.time()<deadline:
            try:
                with open(acknowledgement) as stream: receipt = json.load(stream)
            except (IOError,ValueError): receipt = None
            if receipt==marker:
                delivered.append(dict(marker=marker)); return result
            time.sleep(.01)
        raise ValueError('missing owned mesh cancellation receipt')
    finally:
        event -= ended


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System = host['Rhino'],host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('ScaleByPlane Object requires idle execution')
    if list(doc.Objects): raise ValueError('ScaleByPlane Object requires an empty owned document')
    from join_probe import observe_command
    vp = doc.Views.ActiveView.ActiveViewport
    old = vp.GetConstructionPlane()
    ids,target_id = [],None
    frame = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),
            Rhino.Geometry.Vector3d(1.,1.,0.),Rhino.Geometry.Vector3d(-1.,1.,2.)) if op['plane']=='tilted' else Rhino.Geometry.Plane.WorldXY
    def point(coords):
        return frame.Origin+coords[0]*frame.XAxis+coords[1]*frame.YAxis+coords[2]*frame.ZAxis
    def plane_value(plane):
        return dict(origin=host['_xyz'](plane.Origin),x_axis=host['_xyz'](plane.XAxis),y_axis=host['_xyz'](plane.YAxis))
    def geometry_value(geometry):
        value = dict(kind=geometry.GetType().Name)
        if isinstance(geometry,Rhino.Geometry.Point): value['point'] = host['_xyz'](geometry.Location)
        elif isinstance(geometry,Rhino.Geometry.Curve): value['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
        elif isinstance(geometry,Rhino.Geometry.Brep): value['surface'] = host['_nurbs_surface_definition'](geometry.Surfaces[0].ToNurbsSurface())
        elif isinstance(geometry,Rhino.Geometry.Mesh): value['mesh'] = host['_polygon_mesh_value'](geometry)
        else: raise ValueError('unexpected ScaleByPlane Object geometry')
        return value
    def snapshot():
        return [dict(role='source' if obj.Id in ids else 'output',name=obj.Attributes.Name,
                     selected=bool(obj.IsSelected(False)),point=host['_xyz'](obj.Geometry.Location))
                for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber) if obj.Id!=target_id]
    target = None
    try:
        kind = op['target']
        if kind=='line': target = Rhino.Geometry.LineCurve(point([0.,0.,0.]),point([4.,3.,0.]))
        elif kind=='polyline': target = Rhino.Geometry.PolylineCurve([point(v) for v in ([0.,0.,0.],[4.,1.,0.],[3.,5.,0.],[-1.,3.,0.])])
        elif kind in ('rational','nonplanar_curve'):
            controls = [[-2.,-1.,0.],[0.,3.,0.],[4.,2.,1. if kind=='nonplanar_curve' else 0.],[3.,-1.,0.]]
            target = Rhino.Geometry.NurbsCurve(3,True,4,4)
            for i,(coords,weight) in enumerate(zip(controls,[1.,2.,.5,1.])): target.Points.SetPoint(i,point(coords),weight)
            for i,value in enumerate([0.,0.,0.,1.,1.,1.]): target.Knots[i] = value
        elif kind=='circle': target = Rhino.Geometry.Circle(frame,2.).ToNurbsCurve()
        elif kind=='arc': target = Rhino.Geometry.Arc(Rhino.Geometry.Circle(frame,2.),math.pi*.75).ToNurbsCurve()
        elif kind=='ellipse': target = Rhino.Geometry.Ellipse(frame,3.,2.).ToNurbsCurve()
        elif kind in ('surface','skew_surface','warped_surface'):
            corners = [[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]] if kind=='surface' else [[0.,0.,0.],[4.,1.,0.],[5.,4.,.2 if kind=='warped_surface' else 0.],[-1.,4.,0.]]
            target = Rhino.Geometry.NurbsSurface.CreateFromCorners(*[point(v) for v in corners])
        elif kind=='mesh':
            target = Rhino.Geometry.Mesh(); target.Vertices.UseDoublePrecisionVertices = True
            for v in [[0.,0.,0.],[4.,1.,0.],[5.,4.,0.],[-1.,4.,0.]]:
                p = point(v); target.Vertices.Add(p.X,p.Y,p.Z)
            target.Faces.AddFace(0,1,2,3)
        else: target = Rhino.Geometry.Point(point([1.,1.,0.]))
        if target is None or not target.IsValid: raise ValueError('ScaleByPlane Object target construction failed')
        if op['reverse']:
            if isinstance(target,Rhino.Geometry.Curve): target.Reverse()
            elif isinstance(target,Rhino.Geometry.Surface): target.Reverse(0,True)
            elif isinstance(target,Rhino.Geometry.Mesh): target.Flip(True,True,True)
        serial = doc.BeginUndoRecord('ScaleByPlane Object sources')
        try:
            for i,coords in enumerate([[2.,3.,4.],[2.,2.,3.],[1.,3.,3.],[1.,2.,4.]]):
                attributes = Rhino.DocObjects.ObjectAttributes(); attributes.Name = 'plane-object source '+str(i)
                source_id = doc.Objects.AddPoint(host['_point'](coords),attributes)
                if source_id==System.Guid.Empty: raise ValueError('ScaleByPlane Object source insertion failed')
                ids.append(source_id)
            target_id = doc.Objects.Add(target)
            if target_id==System.Guid.Empty: raise ValueError('ScaleByPlane Object target insertion failed')
        finally: doc.EndUndoRecord(serial)
        target_geometry = doc.Objects.FindId(target_id).Geometry
        target_value = geometry_value(target_geometry)
        sdk = None
        if isinstance(target_geometry,Rhino.Geometry.Curve):
            ok,plane = target_geometry.TryGetPlane(doc.ModelAbsoluteTolerance)
            sdk = dict(available=bool(ok),plane=plane_value(plane) if ok else None)
        elif isinstance(target_geometry,Rhino.Geometry.Brep):
            ok,plane = target_geometry.Faces[0].TryGetPlane(doc.ModelAbsoluteTolerance)
            sdk = dict(available=bool(ok),plane=plane_value(plane) if ok else None)
        active_plane = Rhino.Geometry.Plane(host['_point']([0.,0.,0.]),
            Rhino.Geometry.Vector3d(1.,1.,0.),Rhino.Geometry.Vector3d(-1.,1.,2.)) if op['cplane']=='tilted' else Rhino.Geometry.Plane.WorldXY
        vp.SetConstructionPlane(active_plane)
        # Independent public CPlane Object command; its axes are evidence for
        # comparison, never substituted for an observed ScaleByPlane result.
        marker = 'Viboceros Object CPlane '+str(System.Guid.NewGuid()); Rhino.RhinoApp.WriteLine(marker)
        cplane_macro = '_CPlane _Object _SelID '+str(target_id)+' _Cancel'
        cplane_success,cplane,cplane_events = observe_command(Rhino.Commands.Command,'CPlane',
            lambda:Rhino.RhinoApp.RunScript(cplane_macro,True),lambda:plane_value(vp.GetConstructionPlane().Plane),lambda:[],True)
        cplane_history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        if Rhino.Commands.Command.InCommand(): raise ValueError('Object CPlane witness remains active')
        vp.SetConstructionPlane(active_plane)
        for source_id in ids: doc.Objects.Select(source_id)
        origin,reference,destination = [1.,2.,3.],[3.,5.,8.],[5.,11.,1.]
        macro = '_ScaleByPlane _Copy=_'+('Yes' if op['copy'] else 'No')+' _Rigid=_No _Plane=_Object _SelID '+str(target_id)
        # Native SelID rejects a whole mesh. End at that prompt so subsequent
        # coordinates cannot accidentally act as object picks or shifted input.
        if kind!='mesh':
            macro += ' '+' '.join(token(p) for p in (origin,reference,destination))+(' _Enter' if op['copy'] else '')
        macro += ' _Cancel'
        before = snapshot(); marker = 'Viboceros ScaleByPlane Object '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker); host['_record_progress'](op['id']+' '+macro)
        cancel_inputs = []
        action = (lambda:cancel_mesh(op,macro,host,cancel_inputs)) if kind=='mesh' else lambda:Rhino.RhinoApp.RunScript(macro,True)
        success,after,events = observe_command(Rhino.Commands.Command,'ScaleByPlane',action,snapshot,lambda:[],True)
        if Rhino.Commands.Command.InCommand(): raise ValueError('ScaleByPlane Object macro remains active')
        after_script = snapshot(); history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        selection_history = history.split(str(target_id),1)[-1]
        object_accepted = 'No objects added to selection.' not in selection_history and 'Origin point' in selection_history
        if object_accepted!=(kind!='mesh'):
            raise ValueError('unexpected ScaleByPlane Object acceptance; retain a diagnostic pilot')
        Rhino.RhinoApp.RunScript('_Undo',False); undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False); redo = snapshot()
        return dict(target=target_value,target_frame=plane_value(frame),sdk_plane=sdk,
            active_plane=plane_value(active_plane),object_accepted=object_accepted,
            cancel_inputs=cancel_inputs,
            cplane_macro=cplane_macro,cplane_success=cplane_success,cplane=cplane,
            cplane_events=cplane_events,cplane_history=cplane_history,
            origin=origin,reference=reference,destination=destination,macro=macro,
            before=before,after=after,after_script=after_script,undo=undo,redo=redo,
            events=events,history=history,success=success),0
    finally:
        if target is not None: target.Dispose()
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        vp.SetConstructionPlane(old); doc.Views.Redraw()
