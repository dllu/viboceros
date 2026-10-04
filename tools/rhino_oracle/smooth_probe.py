"""Closed public Smooth command recipes in owned idle documents."""
import re

SOURCES = ('curve','rational','closed','periodic','line','polyline','circle','arc',
           'surface','surface_closed','periodic_cubic','mesh_grid','mesh_triangles',
           'mesh_unwelded','surface_periodic','surface_closed_both')
MODES = {}
for name,factor,steps,axes,fix,coordinates in (
    ('defaults',.2,1,(True,True,True),True,'World'),
    ('free',.2,1,(True,True,True),False,'World'),
    ('zero',0.,1,(True,True,True),True,'World'),
    ('steps',.25,3,(True,True,True),False,'World'),
    ('negative',-.3,1,(True,True,True),False,'World'),
    ('overshoot',1.2,1,(True,True,True),False,'World'),
    ('none',.2,1,(False,False,False),False,'World'),
    ('x',.2,1,(True,False,False),False,'World'),
    ('cplane',.2,1,(True,False,False),False,'CPlane'),
    ('object',.2,1,(True,True,True),False,'Object'),
    ('object_x',.2,1,(True,False,False),False,'Object'),
    ('object_y',.2,1,(False,True,False),False,'Object'),
    ('object_z',.2,1,(False,False,True),False,'Object'),
    ('object_steps',.25,3,(True,False,False),False,'Object')):
    MODES[name] = '_SmoothFactor='+str(factor)+' _Steps='+str(steps)+' _CoordinateSystem=_'+coordinates
    MODES[name] += ' '+ ' '.join('_'+axis+'=_'+('Yes' if enabled else 'No') for axis,enabled in zip(('X','Y','Z'),axes))
    MODES[name] += ' _FixBoundaries=_'+('Yes' if fix else 'No')+' _Enter'



def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=96):
        raise ValueError('Smooth requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','source','selection','mode'}
                or op['op']!='smooth_command' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen
                or op['source'] not in SOURCES or op['selection'] not in ('objects','grips','parent','allgrips','command_first')
                or not isinstance(op['mode'],str) or op['mode'] not in MODES):
            raise ValueError('invalid Smooth recipe')
        seen.add(op['id'])


def request():
    """Fixed-coordinate coverage, including selected grips and native aliases."""
    rows = [(s,'objects',m) for s in SOURCES for m in ('defaults','free','steps','cplane')]
    rows += [(s,p,'free') for s in ('curve','rational','closed','periodic_cubic',
        'surface','surface_closed','mesh_grid','mesh_unwelded') for p in ('grips','parent','allgrips')]
    rows += [(s,'objects',m) for s,m in (('curve','negative'),('curve','overshoot'),
        ('curve','zero'),('curve','none'),('rational','x'),('surface','negative'),('mesh_grid','overshoot'))]
    rows.append(('curve','command_first','defaults'))
    q = dict(protocol_version=1,iterations=1,operations=[dict(op='smooth_command',
        id='smooth-fixed-'+str(i),source=s,selection=p,mode=m) for i,(s,p,m) in enumerate(rows)])
    validate_request(q)
    return q


def create_source(kind,host):
    Rhino = host['Rhino']
    if kind=='mesh_unwelded':
        from grip_transform_probe import create_source as source
        return source('mesh',host)
    if kind in ('surface_periodic','surface_closed_both'):
        import math
        periodic = kind=='surface_periodic'
        nu,nv = (6,3) if periodic else (5,5)
        surface = Rhino.Geometry.NurbsSurface.Create(3,False,3,3,nu,nv)
        for u in range(nu):
            for v in range(nv):
                a,b = (u%4)*math.pi/2,(v%4)*math.pi/2
                p = [2*math.cos(a),2*math.sin(a),float(v)] if periodic else [(3+math.cos(b))*math.cos(a),(3+math.cos(b))*math.sin(a),math.sin(b)]
                # Exact coincident closing seams, without trigonometric drift.
                p = [round(x,14) for x in p]
                surface.Points.SetPoint(u,v,host['_point'](p),1.)
        if periodic: surface.KnotsU.CreatePeriodicKnots(1.)
        else: surface.KnotsU.CreateUniformKnots(1.)
        surface.KnotsV.CreateUniformKnots(1.)
        return surface
    if kind.startswith('mesh_'):
        mesh = Rhino.Geometry.Mesh(); mesh.Vertices.UseDoublePrecisionVertices = True
        for v in range(3):
            for u in range(3): mesh.Vertices.Add(host['_point']([float(u),float(v),4. if (u,v)==(1,1) else .2*u*v]))
        for v in range(2):
            for u in range(2):
                a = v*3+u
                if kind=='mesh_triangles':
                    mesh.Faces.AddFace(a,a+1,a+4); mesh.Faces.AddFace(a,a+4,a+3)
                else: mesh.Faces.AddFace(a,a+1,a+4,a+3)
        return mesh
    if kind=='periodic_cubic':
        points = [[2.,0.,1.],[0.,2.,-2.],[-2.,0.,3.],[0.,-2.,-1.]]
        curve = Rhino.Geometry.NurbsCurve(3,True,4,7)
        for i,p in enumerate(points+points[:3]): curve.Points.SetPoint(i,host['_point'](p),1.)
        curve.Knots.CreatePeriodicKnots(1.)
        return curve
    from grip_transform_probe import create_source as source
    geometry = source(kind,host)
    if kind=='surface':
        c = geometry.Points.GetControlPoint(1,1)
        geometry.Points.SetPoint(1,1,c.Location+Rhino.Geometry.Vector3d(0.,0.,2.),c.Weight)
    return geometry


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System = host['Rhino'],host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('Smooth requires idle execution')
    if list(doc.Objects): raise ValueError('Smooth requires an empty owned document')
    from join_probe import observe_command
    vp = doc.Views.ActiveView.ActiveViewport; old = vp.GetConstructionPlane()
    plane = Rhino.Geometry.Plane(host['_point']([5.,-4.,2.]),Rhino.Geometry.Vector3d(1.,1.,0.),Rhino.Geometry.Vector3d(-1.,1.,2.))
    vp.SetConstructionPlane(plane)
    source_id = None
    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            geometry = obj.Geometry
            row = dict(kind=geometry.GetType().Name,name=obj.Attributes.Name,selected=bool(obj.IsSelected(False)),
                grips_on=bool(obj.GripsOn),grips=[dict(index=int(g.Index),point=host['_xyz'](g.CurrentLocation),
                    selected=bool(g.IsSelected(False))) for g in (obj.GetGrips() or [])])
            if isinstance(geometry,Rhino.Geometry.Curve): row['curve'] = host['_nurbs_curve_definition'](geometry.ToNurbsCurve())
            elif isinstance(geometry,Rhino.Geometry.Brep): row['surface'] = host['_nurbs_surface_definition'](geometry.Surfaces[0].ToNurbsSurface())
            elif isinstance(geometry,Rhino.Geometry.Mesh): row['mesh'] = host['_polygon_mesh_value'](geometry)
            else: raise ValueError('unexpected Smooth geometry')
            rows.append(row)
        return rows
    try:
        serial = doc.BeginUndoRecord('Smooth source creation')
        try:
            geometry = create_source(op['source'],host)
            try:
                attributes = Rhino.DocObjects.ObjectAttributes(); attributes.Name = 'smooth source'
                source_id = doc.Objects.Add(geometry,attributes)
                if source_id==System.Guid.Empty: raise ValueError('Smooth source insertion failed')
            finally: geometry.Dispose()
        finally: doc.EndUndoRecord(serial)
        source = doc.Objects.FindId(source_id)
        if op['selection'] in ('grips','parent','allgrips'):
            source.GripsOn = True; grips = source.GetGrips()
            if not grips: raise ValueError('Smooth source grips unavailable')
            for g in grips:
                if op['selection']=='allgrips' or int(g.Index) in (0,1): g.Select(True)
        if op['selection'] in ('objects','parent'): doc.Objects.Select(source_id)
        macro = '_-Smooth '+('_SelAll _Enter ' if op['selection']=='command_first' else '')+MODES[op['mode']]+' _Cancel _Cancel'
        before = snapshot(); marker = 'Viboceros Smooth '+str(System.Guid.NewGuid()); Rhino.RhinoApp.WriteLine(marker)
        host['_record_progress'](op['id']+' '+macro)
        success,after,events = observe_command(Rhino.Commands.Command,'Smooth',
            lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if not success or [e.get('result') for e in events if e['name']=='Smooth'] != ['Success']:
            raise ValueError('Smooth did not end successfully')
        if Rhino.Commands.Command.InCommand(): raise ValueError('Smooth macro remains active')
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        after_script = snapshot()
        Rhino.RhinoApp.RunScript('_Undo',False); undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo',False); redo = snapshot()
        return dict(before=before,after=after,after_script=after_script,undo=undo,redo=redo,
            macro=macro,success=success,events=events,history=history,
            plane=dict(origin=host['_xyz'](plane.Origin),x_axis=host['_xyz'](plane.XAxis),y_axis=host['_xyz'](plane.YAxis)),
            command_registered='Smooth' in list(Rhino.Commands.Command.GetCommandNames(True,False))),0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn: obj.GripsOn=False
            doc.Objects.Delete(obj.Id,True)
        vp.SetConstructionPlane(old); doc.Views.Redraw()
