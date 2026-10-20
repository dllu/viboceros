"""Read-only public SDK observations of surface and composed curve soft edits."""
import math
import re


def validate(op):
    required={'op','id','artifact_path','reference','target','distance'}
    if not isinstance(op,dict)or not required<=set(op)or set(op)-required-{'uv','fix_edges'}or op.get('op')!='soft_edit_surface_sdk':raise ValueError('invalid surface SDK recipe')
    if not isinstance(op['id'],str)or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id'])is None:raise ValueError('invalid SDK identity')
    if not isinstance(op['artifact_path'],str)or not op['artifact_path'].endswith('.3dm'):raise ValueError('invalid SDK source')
    for name,size in [('reference',3),('target',3),('distance',2),('uv',2)]:
        if name not in op:continue
        values=op[name]
        if not isinstance(values,list)or len(values)!=size or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in values):raise ValueError('invalid SDK values')
    if any(x<=0 for x in op['distance'])or type(op.get('fix_edges',False))is not bool:raise ValueError('invalid SDK soft edit options')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one SDK evaluation required')
    R=host['Rhino'];G=R.Geometry;owned=[]
    def keep(g):
        if g is None:raise ValueError('null SDK geometry')
        owned.append(g);return g
    path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
    file=R.FileIO.File3dm.Read(path)
    if file is None:raise ValueError('source read failed')
    def definition(s):return host['_nurbs_surface_definition'](s)
    def distance(a,b):return math.sqrt(sum((x-y)**2 for x,y in zip(a,b)))
    def curve_factors(curve,t,length,fix):
        n=keep(curve.ToNurbsCurve());data=host['_nurbs_curve_definition'](n)
        count=n.Points.Count;degree=n.Degree;knots=data['knots'];domain=n.Domain
        unique=count-degree if n.IsPeriodic else count-1 if n.IsClosed else count
        factors=[0.]*count
        for i in range(count):
            site=sum(knots[i+1:i+degree+1])/degree
            if not domain.T0<=site<=domain.T1 or fix and not n.IsPeriodic and i in(0,count-1):continue
            distance=0. if site==t else n.GetLength(1e-12,G.Interval(min(site,t),max(site,t)))
            factors[i%unique]+=math.exp(-4.5*(distance/length)**2)
        for i in range(unique,count):factors[i]=factors[i%unique]
        return factors
    try:
        objects=list(file.Objects)
        if len(objects)!=1:raise ValueError('expected one SDK source')
        b=objects[0].Geometry;surface=b.Faces[0].UnderlyingSurface()if isinstance(b,G.Brep)else b
        n=keep(surface.ToNurbsSurface());d=definition(n);nu,nv=d['control_count'];pu,pv=d['degree']
        ok,raw_u,raw_v=n.ClosestPoint(G.Point3d(*op['reference']))
        if not ok:raise ValueError('source projection failed')
        sites=[[sum(d[key][i+1:i+degree+1])/degree for i in range(count)]for key,degree,count in [('knots_u',pu,nu),('knots_v',pv,nv)]]
        markers=[[t for t in ts if n.Domain(axis).T0<=t<=n.Domain(axis).T1]for axis,ts in enumerate(sites)]
        uv=op.get('uv')or[min(markers[0],key=lambda t:abs(t-raw_u)),min(markers[1],key=lambda t:abs(t-raw_v))]
        anchor=n.PointAt(*uv);delta=G.Point3d(*op['target'])-anchor
        result=keep(G.Surface.CreateSoftEditSurface(n,G.Point2d(*uv),delta,op['distance'][0],op['distance'][1],tolerance['absolute'],op.get('fix_edges',False)));result=keep(result.ToNurbsSurface());want=definition(result)
        bresult=keep(result.ToBrep())
        vf=curve_factors(keep(n.IsoCurve(1,uv[0])),uv[1],op['distance'][1],False)
        variants=[]
        for mode in('raw','clamp','wrap','endalias','projected_curve','projected_surface'):
            q=keep(n.Duplicate());q=keep(q.ToNurbsSurface());rows=[];error=None
            order=list(range(nv))
            if mode=='endalias':order=list(range(1,nv))+[0]
            v_iso=keep(n.IsoCurve(1,uv[0]))
            projected=[]
            for j,t in enumerate(sites[1]):
                p=n.PointAt(uv[0],t);ok,cu,cv=n.ClosestPoint(p);curve_ok,ct=v_iso.ClosestPoint(v_iso.PointAt(t));projected.append(dict(index=j,surface_parameter=[cu,cv],curve_parameter=ct))
            for j in order:
                t=sites[1][j]
                if op.get('fix_edges')and j in(0,nv-1):continue
                domain=n.Domain(1)
                t_used=max(domain.T0,min(domain.T1,t))if mode=='clamp'else (domain.T0+(t-domain.T0)%domain.Length if mode=='wrap'and n.IsClosed(1)else t)
                if mode=='endalias'and j in(0,nv-1):t_used=domain.T1
                if mode=='projected_curve':t_used=projected[j]['curve_parameter']
                if mode=='projected_surface':t_used=projected[j]['surface_parameter'][1]
                iso=q.IsoCurve(0,t_used)
                if iso is None:error='null isocurve at '+str(t_used);break
                keep(iso);curve=keep(iso.ToNurbsCurve());before=host['_nurbs_curve_definition'](curve);uf=curve_factors(curve,uv[0],op['distance'][0],op.get('fix_edges',False))
                rows.append(dict(index=j,raw_parameter=t,parameter=t_used,curve=before,factors=uf))
                for i in range(nu):
                    if op.get('fix_edges')and i in(0,nu-1):continue
                    cv=q.Points.GetControlPoint(i,j);p=cv.Location+delta*(vf[j]*uf[i])
                    if not q.Points.SetPoint(i,j,p,cv.Weight):raise ValueError('SDK row update failed')
            actual=definition(q);gap=max(distance(a['point'],b['point'])for a,b in zip(actual['control_points'],want['control_points']))
            variants.append(dict(mode=mode,error=error,maximum_error=gap,definition=actual,rows=rows,projected=projected))
        iso_rows=[]
        for t in sites[1]:
            c=n.IsoCurve(0,t)
            iso_rows.append(dict(parameter=t,definition=host['_nurbs_curve_definition'](keep(c))if c is not None else None))
        return dict(source=d,raw_parameter=[raw_u,raw_v],parameter=uv,anchor=host['_xyz'](anchor),delta=host['_xyz'](delta),source_closed=[bool(n.IsClosed(i))for i in(0,1)],source_periodic=[bool(n.IsPeriodic(i))for i in(0,1)],v_factors=vf,result=want,result_closed=[bool(result.IsClosed(i))for i in(0,1)],result_valid=bool(result.IsValid),brep_valid=bool(bresult.IsValid),brep_vertex_count=int(bresult.Vertices.Count),brep_edge_count=int(bresult.Edges.Count),doc_tolerance=float(R.RhinoDoc.ActiveDoc.ModelAbsoluteTolerance),source_iso_rows=iso_rows,variants=variants),0
    finally:
        for g in reversed(owned):g.Dispose()
        file.Dispose()
