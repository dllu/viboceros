"""Bounded finite NURBS recipes shared by standalone SDK workers."""
import math

def finite(x):
    return type(x) in (int,float) and not math.isnan(x) and not math.isinf(x)

def vector(p):
    return isinstance(p,list) and len(p)==3 and all(finite(x) for x in p)

def validate_curve(c,degree_limit=16,control_limit=128):
    if (not isinstance(c,dict) or not {'degree','control_points','knots'}<=set(c)
            or set(c)-{'degree','control_points','knots','domain'} or type(c['degree']) is not int
            or not 1<=c['degree']<=degree_limit or not isinstance(c['control_points'],list)
            or not c['degree']<len(c['control_points'])<=control_limit):
        raise ValueError('invalid curve recipe layout')
    n=len(c['control_points']);p=c['degree'];k=c['knots'];cp=c['control_points']
    if (not isinstance(k,list) or len(k)!=n+p+1 or not all(finite(x) for x in k)
            or any(a>b for a,b in zip(k,k[1:])) or not k[p]<k[n]
            or any(not isinstance(q,dict) or set(q)!={'point','weight'} or not vector(q['point'])
                   or not finite(q['weight']) or q['weight']==0 for q in cp)):
        raise ValueError('invalid curve recipe data')
    if any((q['weight']>0)!=(cp[0]['weight']>0) for q in cp):
        raise ValueError('curve weights must have a common sign')
    if 'domain' in c:
        d=c['domain']
        if not isinstance(d,list) or len(d)!=2 or not all(finite(x) for x in d) or not d[0]<d[1]:
            raise ValueError('invalid curve recipe domain')
    return n
