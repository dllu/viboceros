"""Owned File3dm transfer with public nested NURBS representation queries."""
import re
FAMILIES=('polycurve','nested_polycurve','face','box')
MASKS=('polynomial','mixed','rational')

def validate(op):
    required={'op','id','mode','artifact_path'}
    if (not isinstance(op,dict) or not required<=set(op) or op['op']!='nested_rational_transfer'
        or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id'])is None
        or op['mode']not in('create','read') or not isinstance(op['artifact_path'],str)
        or not op['artifact_path'].endswith('.3dm')):raise ValueError('invalid nested NURBS transfer recipe')
    if op['mode']=='read':
        if set(op)!=required:raise ValueError('readback does not construct geometry')
    elif (set(op)!=required|{'family','mask','gauge'} or op['family']not in FAMILIES
          or op['mask']not in MASKS or type(op['gauge'])not in(int,float)or op['gauge']not in(1.,7.)):
        raise ValueError('invalid bounded nested NURBS source')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('nested transfer capture requires one iteration')
    R=host['Rhino'];G=R.Geometry;owned=[]
    def keep(c):
        if c is None or not c.IsValid:raise ValueError('invalid owned source geometry')
        owned.append(c);return c
    def point(p):return host['_xyz'](p)
    def curve(c):
        v=host['_interchange_curve_record'](c)
        if isinstance(c,G.NurbsCurve):v['rational']=bool(c.IsRational)
        if isinstance(c,G.PolyCurve):
            flat=keep(c.DuplicateCurve());flat.RemoveNesting()
            v=host['_interchange_curve_record'](flat)
            v['segments']=[curve(flat.SegmentCurve(i))for i in range(flat.SegmentCount)]
        return v
    def record(g):
        if isinstance(g,G.Curve):return dict(kind='curve',geometry=curve(g))
        if not isinstance(g,G.Brep):raise ValueError('unexpected readback geometry')
        edges=[]
        for e in g.Edges:
            n=keep(e.ToNurbsCurve());edges.append(dict(vertices=[int(e.StartVertex.VertexIndex),int(e.EndVertex.VertexIndex)],curve=curve(n)))
        faces=[]
        for f in g.Faces:
            n=keep(f.UnderlyingSurface().ToNurbsSurface())
            faces.append(dict(reversed=bool(f.OrientationIsReversed),definition=host['_nurbs_surface_definition'](n),loops=[dict(type=str(l.LoopType),trims=[dict(edge=int(t.Edge.EdgeIndex)if t.Edge else None,reversed=bool(t.IsReversed()),definition=host['_nurbs_curve_definition'](keep(t.ToNurbsCurve())))for t in l.Trims])for l in f.Loops]))
        return dict(kind='brep',valid=bool(g.IsValid),solid=bool(g.IsSolid),vertices=[point(v.Location)for v in g.Vertices],edges=edges,faces=faces)
    def rational(index):return op['mask']=='rational'or(op['mask']=='mixed'and index%2==0)
    def setup(n,index):
        if rational(index):
            if not n.Points.MakeRational():raise ValueError('MakeRational failed')
            if op['gauge']!=1.:
                for i in range(n.Points.Count):
                    cv=n.Points[i]
                    if not n.Points.SetPoint(i,cv.Location,op['gauge']):raise ValueError('weight assignment failed')
        elif not n.Points.MakeNonRational():raise ValueError('MakeNonRational failed')
        if bool(n.IsRational)!=rational(index):raise ValueError('source representation did not stick')
        return n
    path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
    try:
        if op['mode']=='create':
            family=op['family']
            if family in('polycurve','nested_polycurve'):
                parts=[]
                for i in range(3):
                    x=3.*i
                    n=keep(host['_nurbs_curve_from_definition'](dict(degree=3,knots=[-2.]*4+[4.]*4,control_points=[dict(point=p,weight=1.)for p in[[x,0.,0.],[x+1.,2.,0.],[x+2.,-1.,0.],[x+3.,0.,0.]]])))
                    parts.append(setup(n,i))
                source=G.PolyCurve();owned.append(source)
                if family=='nested_polycurve':
                    child=G.PolyCurve();owned.append(child)
                    if not child.Append(parts[0])or not child.Append(parts[1])or not source.Append(child):raise ValueError('nested append failed')
                    if not source.Append(parts[2]):raise ValueError('last child append failed')
                else:
                    for part in parts:
                        if not source.Append(part):raise ValueError('polycurve append failed')
                line=keep(G.LineCurve(G.Point3d(9.,0.,0.),G.Point3d(12.,0.,0.)))
                if not source.Append(line):raise ValueError('line append failed')
                source.Domain=G.Interval(-5.,19.)
            else:
                if family=='face':
                    s=keep(G.NurbsSurface.CreateFromCorners(G.Point3d(0.,0.,0.),G.Point3d(4.,0.,0.),G.Point3d(4.,3.,0.),G.Point3d(0.,3.,0.)))
                    source=keep(G.Brep.CreateFromSurface(s))
                else:source=keep(G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0.,0.,0.),G.Point3d(4.,3.,2.))))
                for i in range(source.Edges.Count):
                    n=setup(keep(source.Edges[i].ToNurbsCurve()),i);index=source.Curves3D.Add(n)
                    if index<0 or not source.Edges[i].SetEdgeCurve(index):raise ValueError('edge curve assignment failed')
                source.Compact()
            if not source.IsValid:raise ValueError('source failed public IsValid')
            source.SetUserString('GeometryCode','nested')
            model=R.FileIO.File3dm();owned.append(model)
            layer_index=model.Layers.AddDefaultLayer('NestedRational',host['System'].Drawing.Color.Black)
            if layer_index<0:raise ValueError('source layer creation failed')
            a=R.DocObjects.ObjectAttributes();a.Name='NestedSource';a.LayerIndex=layer_index;a.SetUserString('ObjectCode','nested')
            try:
                if isinstance(source,G.Curve):key=model.Objects.AddCurve(source,a)
                else:key=model.Objects.AddBrep(source,a)
            finally:a.Dispose()
            if not model.Write(path,8):raise ValueError('source File3dm.Write failed')
        loaded=R.FileIO.File3dm.Read(path)
        if loaded is None:raise ValueError('File3dm.Read failed')
        owned.append(loaded);objects=list(loaded.Objects)
        if len(objects)!=1:raise ValueError('expected one source object')
        obj=objects[0]
        return dict(geometry=record(obj.Geometry),name=obj.Attributes.Name,attribute_text=obj.Attributes.GetUserString('ObjectCode'),geometry_text=obj.Geometry.GetUserString('GeometryCode')),0
    finally:
        for c in reversed(owned):c.Dispose()
