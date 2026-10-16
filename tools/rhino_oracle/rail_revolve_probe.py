"""Owned public SDK rail-revolution construction and world-space queries."""
import json
import math
import os
try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)


def request():
    with open(os.path.join(os.path.dirname(__file__), 'fixtures', 'rail_revolve.json')) as stream:
        return json.load(stream)


def finite(x):
    return type(x) in (int, float) and not math.isnan(x) and not math.isinf(x)


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op','id','profile','rail','axis','scale_height','queries'}
            or op['op'] != 'rail_revolve' or not isinstance(op['id'], string_types)
            or type(op['scale_height']) is not bool or not isinstance(op['axis'], list) or len(op['axis']) != 2
            or not isinstance(op['queries'], list) or not 1 <= len(op['queries']) <= 64):
        raise ValueError('invalid rail revolution fields')
    def vector(p): return isinstance(p, list) and len(p) == 3 and all(finite(x) for x in p)
    if not all(vector(p) for p in op['axis']+op['queries']) or op['axis'][0] == op['axis'][1]:
        raise ValueError('invalid rail revolution axis or queries')
    sizes=[]
    for c in [op['profile'],op['rail']]:
        if (not isinstance(c,dict) or not {'degree','control_points','knots'} <= set(c)
                or set(c)-{'degree','control_points','knots','domain'} or type(c['degree']) is not int
                or not 1 <= c['degree'] <= 64 or not isinstance(c['control_points'],list)
                or not c['degree'] < len(c['control_points']) <= 100000):
            raise ValueError('invalid rail revolution curve layout')
        n=len(c['control_points']); p=c['degree']; k=c['knots']
        if (not isinstance(k,list) or len(k) != n+p+1 or not all(finite(x) for x in k)
                or any(a>b for a,b in zip(k,k[1:])) or not k[p] < k[n]
                or any(not isinstance(q,dict) or set(q) != {'point','weight'} or not vector(q['point'])
                       or not finite(q['weight']) or q['weight'] == 0 for q in c['control_points'])):
            raise ValueError('invalid rail revolution curve data')
        weights=[q['weight'] for q in c['control_points']]
        if any((w>0) != (weights[0]>0) for w in weights):
            raise ValueError('rail revolution requires weights with a common sign')
        if c.get('domain') is not None:
            d=c['domain']
            if not isinstance(d,list) or len(d)!=2 or not all(finite(x) for x in d) or not d[0]<d[1]:
                raise ValueError('invalid rail revolution curve domain')
        sizes.append(n)
    if sizes[0]*sizes[1] > 1000000:
        raise ValueError('rail revolution control-net limit')


def run(op,host,iterations,tolerance):
    validate(op)
    G=host['Rhino'].Geometry
    sources=[]
    try:
        for definition in [op['profile'],op['rail']]:
            c=host['_nurbs_curve_from_definition'](definition)
            if c is None: raise ValueError('SDK rail revolution curve construction failed')
            sources.append(c)
        axis=G.Line(host['_point'](op['axis'][0]),host['_point'](op['axis'][1]))
        queries=[host['_point'](q) for q in op['queries']]
        def compute():
            copies=[];output=None
            try:
                for c in sources:
                    duplicate=c.DuplicateCurve()
                    if duplicate is None or duplicate is c: raise ValueError('SDK rail revolution copy failed')
                    copies.append(duplicate)
                output=G.NurbsSurface.CreateRailRevolvedSurface(copies[0],copies[1],axis,op['scale_height'])
                if output is None:return dict(created=False,valid=False,area=None,closest_points=[])
                closest=[]
                for q in queries:
                    ok,u,v=output.ClosestPoint(q)
                    if not ok:raise ValueError('SDK rail revolution closest point failed')
                    closest.append(host['_xyz'](output.PointAt(u,v)))
                mass=G.AreaMassProperties.Compute(output)
                if mass is None:raise ValueError('SDK rail revolution area failed')
                try:
                    area=float(mass.Area)
                    if not finite(area) or area<=0:raise ValueError('invalid SDK rail revolution area')
                finally:mass.Dispose()
                return dict(created=True,valid=bool(output.IsValid),area=area,closest_points=closest,
                            definition=host['_nurbs_surface_definition'](output))
            finally:
                if output is not None:output.Dispose()
                for c in copies:c.Dispose()
        return host['_measure'](iterations,compute)
    finally:
        for c in sources:c.Dispose()
