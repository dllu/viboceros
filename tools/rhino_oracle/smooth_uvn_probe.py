"""Closed public SDK witnesses for all Smooth Object axes and repeated passes."""
import re
if __package__:
    from .smooth_probe import SOURCES, create_source
else:
    from smooth_probe import SOURCES, create_source


def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=96):
        raise ValueError('Smooth UVN requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','source','mode','graph'}
                or op['op']!='smooth_uvn' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen
                or op['source'] not in SOURCES or op['mode'] not in ('x','y','z','steps')
                or op['graph'] not in ('initial','step','raw_initial')):
            raise ValueError('invalid Smooth UVN recipe')
        seen.add(op['id'])


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino = host['Rhino']; doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('Smooth UVN requires idle execution')
    if list(doc.Objects): raise ValueError('Smooth UVN requires an empty owned document')
    source = create_source(op['source'],host); current = None
    factor,steps,axis = (.25,3,0) if op['mode']=='steps' else (.2,1,('x','y','z').index(op['mode']))
    mesh = isinstance(source,Rhino.Geometry.Mesh); surface = isinstance(source,Rhino.Geometry.Surface)
    def points():
        if mesh: return [current.Vertices.Point3dAt(i) for i in range(current.Vertices.Count)]
        if surface: return [current.Points.GetControlPoint(u,v).Location for v in range(current.Points.CountV) for u in range(current.Points.CountU)]
        return [current.Points[i].Location for i in range(current.Points.Count)]
    def graph():
        if mesh:
            topology = current.TopologyVertices
            return [[min(topology.MeshVertexIndices(j)) for j in topology.ConnectedTopologyVertices(topology.TopologyVertexIndex(i))] for i in range(current.Vertices.Count)]
        if not surface:
            count = current.Points.Count
            unique = count-current.Degree if current.IsPeriodic else count-1 if current.IsClosed and points()[0]==points()[-1] else count
            if op['graph']=='raw_initial' and unique<count:
                return [[unique-1 if i==0 else i-1,count-unique if i+1==count else i+1] for i in range(count)]
            return [[(i%unique+unique-1)%unique,(i%unique+1)%unique] if unique<count else [1] if i==0 else [i-1] if i==count-1 else [i-1,i+1] for i in range(count)]
        nu,nv = current.Points.CountU,current.Points.CountV
        gu = nu-current.Degree(0) if current.IsPeriodic(0) else nu-1 if current.IsClosed(0) else nu
        gv = nv-current.Degree(1) if current.IsPeriodic(1) else nv-1 if current.IsClosed(1) else nv
        rows=[]
        for v in range(nv):
            for u in range(nu):
                a,b=u%gu,v%gv; adjacent=[]
                if op['graph']=='raw_initial':
                    for direction,c,g,n in ((0,u,gu,nu),(1,v,gv,nv)):
                        for d in (-1,1):
                            k=c+d
                            if not 0<=k<n:
                                if g==n: continue
                                k=g-1 if d<0 else n-g
                            adjacent.append(v*nu+k if direction==0 else k*nu+u)
                    rows.append(adjacent)
                    continue
                for direction,c,g,n in ((0,a,gu,nu),(1,b,gv,nv)):
                    for d in (-1,1):
                        k=(c+d)%g if g<n else c+d
                        if 0<=k<g: adjacent.append(b*nu+k if direction==0 else k*nu+a)
                rows.append(adjacent)
        return rows
    try:
        current = source.DuplicateMesh() if mesh else source.ToNurbsSurface() if surface else source.ToNurbsCurve()
        initial_graph = graph()
        mesh_frames=[]
        if mesh:
            current.Normals.ComputeNormals()
            mesh_frames = [Rhino.Geometry.Plane(p,Rhino.Geometry.Vector3d(current.Normals[i])) for i,p in enumerate(points())]
        rows=[]
        for step in range(steps):
            before = points(); adjacent = graph() if op['graph']=='step' and not mesh else initial_graph
            order = [v*current.Points.CountU+u for u in range(current.Points.CountU) for v in range(current.Points.CountV)] if surface else range(len(before))
            for i in order:
                old = before[i]
                target = Rhino.Geometry.Point3d(*[sum(getattr(before[j],c) for j in adjacent[i])/len(adjacent[i]) for c in ('X','Y','Z')])
                available = True; parameter = None
                if mesh: frame = mesh_frames[i]
                elif surface:
                    u,v = i%current.Points.CountU,i//current.Points.CountU
                    uv = current.Points.GetGrevillePoint(u,v); parameter=[uv.X,uv.Y]
                    available,du,dv,n = current.UVNDirectionsAt(uv.X,uv.Y)
                    if not available:
                        evaluated,p,derivatives = current.Evaluate(uv.X,uv.Y,1)
                        if not evaluated: raise ValueError('surface continuation failed')
                        du,dv=derivatives[0],derivatives[1]; n=Rhino.Geometry.Vector3d.CrossProduct(du,dv)
                    frame = Rhino.Geometry.Plane(old,Rhino.Geometry.Vector3d.CrossProduct(dv,n),dv)
                    if not frame.IsValid: frame = Rhino.Geometry.Plane.WorldXY
                else:
                    t=float(current.GrevilleParameter(i)); parameter=t
                    available,u,v,n = current.UVNDirectionsAt(t)
                    if not available:
                        u=current.TangentAt(t); curvature=current.CurvatureAt(t)
                        n=Rhino.Geometry.Vector3d.CrossProduct(u,curvature)
                    frame = Rhino.Geometry.Plane(old,u,n)
                    if current.Degree==1 or not frame.IsValid: frame = Rhino.Geometry.Plane.WorldXY
                direction=(frame.XAxis,frame.YAxis,frame.ZAxis)[axis]
                point=old+(float((target-old)*direction)*factor)*direction
                if mesh: current.Vertices.SetVertex(i,point)
                elif surface: current.Points.SetPoint(u,v,point,current.Points.GetControlPoint(u,v).Weight)
                else: current.Points.SetPoint(i,point,current.Points[i].Weight)
                rows.append(dict(step=step,index=i,parameter=parameter,available=available,
                    x=host['_xyz'](frame.XAxis),y=host['_xyz'](frame.YAxis),z=host['_xyz'](frame.ZAxis),point=host['_xyz'](point)))
        definition = host['_polygon_mesh_value'](current) if mesh else host['_nurbs_surface_definition'](current) if surface else host['_nurbs_curve_definition'](current)
        return dict(rows=rows,predicted=definition),0
    finally:
        if current is not None: current.Dispose()
        source.Dispose()
