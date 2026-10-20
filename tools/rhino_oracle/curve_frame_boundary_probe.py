"""Bounded, read-only public SDK frame batches across authored corners."""
import math

def validate(op):
    required={'op','id','vertices','representation','parameters','domain'}
    scalar=lambda n:type(n)in(int,float)and not math.isnan(n)and not math.isinf(n)and abs(n)<=1e6
    if not isinstance(op,dict)or set(op)-{'query_mode'}!=required or op['op']!='curve_frame_boundary':raise ValueError('invalid frame boundary recipe')
    if not isinstance(op['id'],(str,type(u'')))or not 1<=len(op['id'])<=64:raise ValueError('invalid frame boundary id')
    if op.get('query_mode','Raw')not in('Raw','Knots'):raise ValueError('invalid frame boundary query mode')
    if op['representation']not in('Polyline','Nurbs','Polycurve','NurbsCubic','Bezier','BezierChain'):raise ValueError('invalid frame boundary representation')
    v=op['vertices'];p=op['parameters'];d=op['domain']
    if not isinstance(v,list)or not 2<=len(v)<=16 or any(not isinstance(q,list)or len(q)!=3 or not all(scalar(n)for n in q)for q in v)or any(a==b for a,b in zip(v,v[1:])):raise ValueError('invalid frame boundary vertices')
    if op['representation']=='Bezier'and len(v)!=4 or op['representation']=='BezierChain'and(len(v)<4 or(len(v)-1)%3):raise ValueError('invalid cubic frame controls')
    if not isinstance(d,list)or len(d)!=2 or not all(scalar(t)for t in d)or d[0]>=d[1]:raise ValueError('invalid frame boundary domain')
    if not isinstance(p,list)or not 2<=len(p)<=128 or any(not scalar(t)for t in p)or any(a>=b for a,b in zip(p,p[1:])):raise ValueError('invalid frame boundary queries')
    if op.get('query_mode')=='Knots':
        spans=1 if op['representation']=='Bezier'else(len(v)-1)//3 if op['representation']=='BezierChain'else len(v)-1
        if any(type(i)is not int or not 0<=i<=spans for i in p):raise ValueError('invalid knot query indices')
    elif any(not d[0]<=t<=d[1]for t in p):raise ValueError('frame query outside domain')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('frame boundary captures require one iteration')
    G=host['Rhino'].Geometry;xyz=host['_xyz'];curve=G.PolylineCurve([G.Point3d(*p)for p in op['vertices']])
    if op['representation']in('Nurbs','NurbsCubic'):
        new=curve.ToNurbsCurve();curve.Dispose();curve=new
        if op['representation']=='NurbsCubic':curve.IncreaseDegree(3)
    elif op['representation']=='Bezier':
        curve.Dispose();curve=G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in op['vertices']],3)
    elif op['representation']=='BezierChain':
        curve.Dispose();curve=G.PolyCurve()
        for i in range(0,len(op['vertices'])-1,3):
            segment=G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in op['vertices'][i:i+4]],3);curve.Append(segment);segment.Dispose()
    elif op['representation']=='Polycurve':
        new=G.PolyCurve()
        for segment in curve.DuplicateSegments():new.Append(segment);segment.Dispose()
        curve.Dispose();curve=new
    curve.Domain=G.Interval(*op['domain'])
    def frame(f):return dict(origin=xyz(f.Origin),x=xyz(f.XAxis),y=xyz(f.YAxis),z=xyz(f.ZAxis))
    try:
        parameters=op['parameters']
        if op.get('query_mode')=='Knots':
            n=curve.ToNurbsCurve()
            try:knots=sorted(set(float(t)for t in n.Knots))
            finally:n.Dispose()
            if max(parameters)>=len(knots):raise ValueError('knot query index exceeds the source spans')
            parameters=[knots[i]for i in parameters]
        batch=curve.GetPerpendicularFrames(parameters);first=parameters[0]
        ok,perp=curve.PerpendicularFrameAt(first);ok_f,frenet=curve.FrameAt(first)
        return dict(parameters=parameters,curve=host['_nurbs_curve_definition'](curve),frames=None if batch is None else[frame(f)for f in batch],perpendicular=frame(perp)if ok else None,frenet=frame(frenet)if ok_f else None,normal_plane=frame(G.Plane(curve.PointAt(first),curve.TangentAt(first)))),0
    finally:curve.Dispose()
