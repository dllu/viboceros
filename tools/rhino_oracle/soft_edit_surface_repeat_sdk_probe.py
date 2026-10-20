"""Read-only public SDK transitions for repeated surface edits."""
import math
import re

def validate(op):
    if not isinstance(op,dict) or set(op)!={'op','id','artifact_path','points','distance'} or op['op']!='soft_edit_surface_repeat_sdk':raise ValueError('invalid surface SDK sequence')
    if not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None:raise ValueError('invalid SDK identity')
    if not isinstance(op['artifact_path'],str) or not op['artifact_path'].endswith('.3dm'):raise ValueError('invalid SDK fixture')
    for name,size in [('distance',2)]:
        if not isinstance(op[name],list) or len(op[name])!=size or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or x<=0 for x in op[name]):raise ValueError('invalid SDK distance')
    if not isinstance(op['points'],list) or len(op['points'])not in(2,4,6):raise ValueError('invalid SDK points')
    for p in op['points']:
        if not isinstance(p,list) or len(p)!=3 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in p):raise ValueError('invalid SDK point')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one SDK sequence required')
    R=host['Rhino'];G=R.Geometry;owned=[]
    def keep(g):
        if g is None:raise ValueError('null SDK surface')
        owned.append(g);return g
    path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
    file=R.FileIO.File3dm.Read(path)
    if file is None:raise ValueError('SDK fixture read failed')
    try:
        n=keep(list(file.Objects)[0].Geometry.Faces[0].ToNurbsSurface());original=n;variants=[]
        for mode in('updated','original_getter'):
            n=original;steps=[]
            for i in range(0,len(op['points']),2):
                getter=n if mode=='updated'else original
                ok,u,v=getter.ClosestPoint(G.Point3d(*op['points'][i]))
                if not ok:raise ValueError('SDK point projection failed')
                definition=host['_nurbs_surface_definition'](getter);sites=[]
                for axis,key in enumerate(('knots_u','knots_v')):
                    degree=definition['degree'][axis];count=definition['control_count'][axis];knots=definition[key];domain=getter.Domain(axis)
                    sites.append([sum(knots[j+1:j+degree+1])/degree for j in range(count)if domain.T0<=sum(knots[j+1:j+degree+1])/degree<=domain.T1])
                uv=[min(sites[0],key=lambda t:abs(t-u)),min(sites[1],key=lambda t:abs(t-v))]
                anchor=getter.PointAt(*uv);delta=G.Point3d(*op['points'][i+1])-anchor
                source=host['_nurbs_surface_definition'](n)
                q=keep(G.Surface.CreateSoftEditSurface(n,G.Point2d(*uv),delta,op['distance'][0],op['distance'][1],tolerance['absolute'],False));q=keep(q.ToNurbsSurface())
                steps.append(dict(source=source,parameter=uv,raw_parameter=[u,v],anchor=host['_xyz'](anchor),delta=host['_xyz'](delta),result=host['_nurbs_surface_definition'](q),source_closed=[bool(n.IsClosed(j))for j in(0,1)],source_periodic=[bool(n.IsPeriodic(j))for j in(0,1)]));n=q
            variants.append(dict(mode=mode,steps=steps))
        return dict(variants=variants),0
    finally:
        for g in reversed(owned):g.Dispose()
        file.Dispose()
