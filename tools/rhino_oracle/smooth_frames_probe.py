"""Independent public SDK direction witnesses for Object coordinate smoothing."""
import re


def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=64):
        raise ValueError('Smooth frames require bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','source','mode'} or op['op']!='smooth_frames'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or op['source'] not in ('curve','rational','closed','periodic','periodic_cubic','line','polyline','circle','arc','surface','surface_closed','mesh_grid','mesh_triangles')
                or op['mode'] not in ('curve','surface_u','surface_v','surface_normal_u','surface_normal_v','mesh_static','mesh_dynamic')):
            raise ValueError('invalid Smooth direction recipe')
        seen.add(op['id'])


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino = host['Rhino']; doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('Smooth frames require idle execution')
    if list(doc.Objects): raise ValueError('Smooth frames require empty owned documents')
    from smooth_probe import create_source
    original = create_source(op['source'],host)
    current,world = None,None
    def xyz(p): return host['_xyz'](p)
    def point(curve,i): return curve.Points[i].Location
    def apply(old,delta,u):
        direction = Rhino.Geometry.Vector3d(u); direction.Unitize()
        displacement = float(delta*direction)*direction
        return old+displacement
    try:
        if op['mode']=='curve':
            current = original.ToNurbsCurve()
            world = current.Smooth(.2,True,True,True,False,Rhino.Geometry.SmoothingCoordinateSystem.World,Rhino.Geometry.Plane.WorldXY)
            if world is None:
                raise ValueError('SDK curve World smoothing failed')
            before = [point(current,i) for i in range(current.Points.Count)]
            changes = [point(world,i)-p for i,p in enumerate(before)]
            rows = []
            for i in range(current.Points.Count):
                t = float(current.GrevilleParameter(i)); available,u,v,n = current.UVNDirectionsAt(t)
                row = dict(index=i,parameter=t,available=available,u=xyz(u),v=xyz(v),n=xyz(n))
                tangent = current.TangentAt(t); row['tangent'] = xyz(tangent)
                if available:
                    location = apply(before[i],changes[i],u)
                    current.Points.SetPoint(i,location,current.Points[i].Weight)
                elif tangent.IsValid and not tangent.IsZero:
                    current.Points.SetPoint(i,apply(before[i],changes[i],tangent),current.Points[i].Weight)
                row['point'] = xyz(point(current,i)); rows.append(row)
            return dict(rows=rows,world=host['_nurbs_curve_definition'](world),
                predicted=host['_nurbs_curve_definition'](current)),0
        if op['mode'].startswith('surface_'):
            current = original.ToNurbsSurface()
            from smooth_probe import run as world_command
            capture,_ = world_command(dict(op='smooth_command',id=op['id']+'-world',source=op['source'],mode='free',selection='objects'),host)
            definition = capture['after'][0]['surface']
            world = host['_nurbs_surface_from_definition'](dict(definition,
                degree_u=definition['degree'][0],degree_v=definition['degree'][1],
                control_point_count_u=definition['control_count'][0],
                control_point_count_v=definition['control_count'][1]))
            nu,nv = current.Points.CountU,current.Points.CountV
            before = dict(((u,v),current.Points.GetControlPoint(u,v).Location) for v in range(nv) for u in range(nu))
            changes = dict(((u,v),world.Points.GetControlPoint(u,v).Location-before[u,v]) for v in range(nv) for u in range(nu))
            order = [(u,v) for u in range(nu) for v in range(nv)] if op['mode'].endswith('_u') else [(u,v) for v in range(nv) for u in range(nu)]
            rows = []
            for u,v in order:
                uv = current.Points.GetGrevillePoint(u,v); available,du,dv,normal = current.UVNDirectionsAt(uv.X,uv.Y)
                row = dict(index=[u,v],parameter=[uv.X,uv.Y],available=available,u=xyz(du),v=xyz(dv),n=xyz(normal))
                if available:
                    if '_normal_' in op['mode']: du = Rhino.Geometry.Vector3d.CrossProduct(dv,normal)
                    current.Points.SetPoint(u,v,apply(before[u,v],changes[u,v],du),current.Points.GetControlPoint(u,v).Weight)
                row['point'] = xyz(current.Points.GetControlPoint(u,v).Location); rows.append(row)
            return dict(rows=rows,world=host['_nurbs_surface_definition'](world),predicted=host['_nurbs_surface_definition'](current)),0
        current = original.DuplicateMesh(); world = current.DuplicateMesh()
        if not world.Smooth(.2,1,True,True,True,False,Rhino.Geometry.SmoothingCoordinateSystem.World,Rhino.Geometry.Plane.WorldXY):
            raise ValueError('SDK mesh World smoothing failed')
        before = [current.Vertices.Point3dAt(i) for i in range(current.Vertices.Count)]
        changes = [world.Vertices.Point3dAt(i)-p for i,p in enumerate(before)]
        current.Normals.ComputeNormals(); rows = []
        for i in range(current.Vertices.Count):
            if op['mode']=='mesh_dynamic':
                current.FaceNormals.Clear(); current.Normals.Clear(); current.Normals.ComputeNormals()
            normal = Rhino.Geometry.Vector3d(current.Normals[i]); frame = Rhino.Geometry.Plane(before[i],normal)
            current.Vertices.SetVertex(i,apply(before[i],changes[i],frame.XAxis))
            rows.append(dict(index=i,u=xyz(frame.XAxis),v=xyz(frame.YAxis),n=xyz(frame.ZAxis),point=xyz(current.Vertices.Point3dAt(i))))
        return dict(rows=rows,world=host['_polygon_mesh_value'](world),predicted=host['_polygon_mesh_value'](current)),0
    finally:
        for value in (world,current,original):
            if value is not None: value.Dispose()
