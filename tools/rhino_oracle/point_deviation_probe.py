"""Public closest-point SDK observations for PointDeviation's distance engine.

This does not capture the native PointDeviation panel, its sampling or settings.
"""
import math
try:
    string_types=(basestring,)
except NameError:
    string_types=(str,)
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_source
else:
    from geometry_validity_probe import validate as validate_source


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','points','targets'} or op['op']!='point_deviation'
        or not isinstance(op['id'],string_types) or not isinstance(op['points'],list) or not 1<=len(op['points'])<=256
        or any(not isinstance(p,list)or len(p)!=3 or any(type(x)not in(int,float)or (math.isnan(x)or math.isinf(x))for x in p)for p in op['points'])
        or not isinstance(op['targets'],list)or not 1<=len(op['targets'])<=16):
        raise ValueError('invalid point deviation recipe')
    for source in op['targets']:
        validate_source(dict(op='geometry_validity',id=op['id'],source=source))
        if source['type']in('point','point_cloud'):raise ValueError('invalid point deviation target')


def run(op,host,iterations,tolerance):
    validate(op);G=host['Rhino'].Geometry;targets=[]
    try:
        for source in op['targets']:targets.append(host['_object_source'](source,tolerance))
        points=[host['_point'](p)for p in op['points']]
        def compute():
            samples=[]
            for p in points:
                best=None
                for i,target in enumerate(targets):
                    if isinstance(target,G.Curve):
                        ok,t=target.ClosestPoint(p)
                        if not ok:raise ValueError('curve closest query failed')
                        q=target.PointAt(t)
                    elif isinstance(target,G.Surface):
                        ok,u,v=target.ClosestPoint(p)
                        if not ok:raise ValueError('surface closest query failed')
                        q=target.PointAt(u,v)
                    elif isinstance(target,G.Brep):q=target.ClosestPoint(p)
                    elif isinstance(target,G.Mesh):
                        location=target.ClosestMeshPoint(p,0.)
                        if location is None:raise ValueError('mesh closest query failed')
                        q=location.Point
                    else:raise ValueError('unsupported closest query')
                    if not q.IsValid:raise ValueError('nonfinite closest query')
                    d=float(p.DistanceTo(q))
                    if best is None or d<best['distance']:best=dict(point=host['_xyz'](q),distance=d,target_index=i)
                samples.append(best)
            d=sorted(s['distance']for s in samples);n=len(d);mean=math.fsum(d)/n
            statistics=dict(count=n,mean=mean,median=(d[(n-1)//2]+d[n//2])/2.,standard_deviation=math.sqrt(math.fsum((v-mean)**2 for v in d)/n),minimum=d[0],maximum=d[-1])
            return dict(samples=samples,statistics=statistics,sdk_only=True)
        return host['_measure'](iterations,compute)
    finally:
        for target in targets:
            if target is not None:target.Dispose()
