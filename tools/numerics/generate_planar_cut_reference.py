"""Emit an independent Fraction reference for finite planar line cut stations."""
from fractions import Fraction as F
import json, math, random
rng=random.Random(8102026)
def relation(a,b):
    a,b=[[list(map(F,p))for p in line]for line in(a,b)]
    r=[a[1][i]-a[0][i]for i in range(2)];s=[b[1][i]-b[0][i]for i in range(2)];o=[b[0][i]-a[0][i]for i in range(2)]
    cross=lambda x,y:x[0]*y[1]-x[1]*y[0]
    d=cross(r,s)
    if d:
        t,u=cross(o,s)/d,cross(o,r)/d
        return dict(kind='point',first=[float(t)],second=[float(u)])if 0<=t<=1 and 0<=u<=1 else dict(kind='none')
    if cross(o,r):return dict(kind='none')
    axis=0 if r[0] else 1
    t0,t1=o[axis]/r[axis],(b[1][axis]-a[0][axis])/r[axis]
    lo,hi=max(F(0),min(t0,t1)),min(F(1),max(t0,t1))
    if lo>hi:return dict(kind='none')
    u0,u1=(r[axis]*lo-o[axis])/s[axis],(r[axis]*hi-o[axis])/s[axis]
    return dict(kind='point'if lo==hi else'overlap',first=[float(lo)]if lo==hi else[float(lo),float(hi)],second=[float(u0)]if lo==hi else[float(u0),float(u1)])
rows=[]
for i in range(192):
    exponent=[-550,-300,0,300,600][i%5]
    scale=math.ldexp(1.,exponent)
    a=[[rng.randint(-10000,10000)*scale for _ in range(2)]for _ in range(2)]
    b=[[rng.randint(-10000,10000)*scale for _ in range(2)]for _ in range(2)]
    rows.append(dict(a=a,b=b,expected=relation(a,b)))
for exponent in[-550,-200,0,200,500]:
    scale=math.ldexp(1.,exponent);n=2.**27
    a=[[0.,0.],[(n+1)*scale,n*scale]];b=[[.5*scale,.5*scale],[(n+.5)*scale,(n-.5)*scale]]
    rows.append(dict(a=a,b=b,expected=relation(a,b)))
for exponent in[-550,0,600]:
    scale=math.ldexp(1.,exponent)
    for b in[[[3.*scale,0.],[scale,0.]],[[4.*scale,0.],[6.*scale,0.]],[[5.*scale,0.],[6.*scale,0.]]]:
        a=[[0.,0.],[4.*scale,0.]];rows.append(dict(a=a,b=b,expected=relation(a,b)))
print(json.dumps(dict(generator='tools/numerics/generate_planar_cut_reference.py',cases=rows),indent=2,allow_nan=False))
