"""Independent Decimal quadrature and analytic first-moment witnesses.

No native implementation or saved observation is read. The bilinear patch is
S(u,v)=(4u,3v,2uv), with density sqrt(144+64u^2+36v^2). Paraboloid
controls use radial analytic antiderivatives for z=x^2+y^2.
"""
from decimal import Decimal as D, localcontext
import json
import math


def legendre(n):
    result=[]
    for i in range(n//2):
        x=D(math.cos(math.pi*(i+.75)/(n+.5)))
        for _ in range(20):
            a,b=D(1),x
            for k in range(2,n+1): a,b=b,((2*k-1)*x*b-(k-1)*a)/k
            derivative=n*(x*b-a)/(x*x-1)
            step=b/derivative
            x-=step
            if abs(step)<D('1e-60'): break
        else: raise ArithmeticError('Legendre root did not converge')
        weight=1/((1-x*x)*derivative*derivative)
        result.extend([((1-x)/2,weight),((1+x)/2,weight)])
    return sorted(result)


def bilinear(n):
    rule=legendre(n)
    integrals=[D(0)]*4
    for u,wu in rule:
        for v,wv in rule:
            weight=wu*wv*(144+64*u*u+36*v*v).sqrt()
            for i,value in enumerate((D(1),4*u,3*v,2*u*v)): integrals[i]+=weight*value
    return dict(area=integrals[0],centroid=[x/integrals[0] for x in integrals[1:]])


def pi():
    def atan_inverse(n):
        x=D(1)/n; power=x; total=x; k=1
        while True:
            power*=-x*x
            term=power/(2*k+1)
            total+=term
            if abs(term)<D('1e-70'): return total
            k+=1
    return 16*atan_inverse(5)-4*atan_inverse(239)


def paraboloid(radius,hole=0.,cap=False):
    r,h=map(D.from_float,(radius,hole))
    def primitive(r):
        u=1+4*r*r
        area=(u*u.sqrt()-1)/6
        moment=(u*u*u.sqrt()/5-u*u.sqrt()/3+D(2)/15)/8
        return area,moment
    a,m=primitive(r); b,n=primitive(h)
    a-=b; m-=n
    if cap: a+=r*r; m+=r**4
    return dict(area=pi()*a,centroid=[D(0),D(0),m/a])


def reference():
    with localcontext() as ctx:
        ctx.prec=68
        low,high=bilinear(32),bilinear(64)
        residual=max(abs(low['area']-high['area']),*(abs(a-b) for a,b in zip(low['centroid'],high['centroid'])))
        if residual>D('1e-40'): raise ArithmeticError('independent bilinear quadrature did not settle')
        result={'shape-4':high}
        for name,r,h,cap in [('disk',.8,0.,False),('annulus',.8,.35,False),('thin-annulus',.8,.799,False),
                             ('capped-outward',.8,0.,True),('capped-inward',.8,0.,True)]:
            result['brep-paraboloid-'+name]=paraboloid(r,h,cap)
        rotated=paraboloid(.5,0.,True)
        rotated['centroid']=[D(8),-4-rotated['centroid'][2],D(16)]
        result['brep-paraboloid-rotated-translated']=rotated
        return dict(bilinear_order_difference=str(residual),values={name:dict(area=str(v['area']),centroid=list(map(str,v['centroid']))) for name,v in result.items()})


if __name__=='__main__': print(json.dumps(reference(),indent=2))
