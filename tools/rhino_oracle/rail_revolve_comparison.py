"""Continuous comparison of positive rational surfaces in matching tensor bases.

Charts may transpose/reverse. With a common nonnegative partition of unity,
control displacement bounds the equal-weight surface displacement. Weight
roundoff contributes at most range(v_i/w_i)/min(v_i/w_i) times the radius of
one control net. All algebra before the outward-rounded square root is exact
rational arithmetic on the recorded binary floating point values.
"""
from fractions import Fraction as F
import math


def _sqrt_up(q):
    r=math.sqrt(float(q))
    if not math.isfinite(r):raise ValueError('nonfinite bound')
    while F(r)**2 < q:r=math.nextafter(r,math.inf)
    return F(r)


def _layout(s):
    count=s['control_count'];degree=s['degree']
    if len(count)!=2 or len(degree)!=2 or any(type(n) is not int or n<1 for n in count+degree):
        raise ValueError('invalid surface layout')
    if any(n<=p for n,p in zip(count,degree)) or len(s['control_points'])!=math.prod(count):
        raise ValueError('invalid surface control count')
    basis=[]
    for axis,n,p in zip(('u','v'),count,degree):
        k=[F(x) for x in s['knots_'+axis]]
        if len(k)!=n+p+1 or any(a>b for a,b in zip(k,k[1:])) or k[n]<=k[p]:
            raise ValueError('invalid surface knots')
        # The first/last full knot is superfluous on the active domain.
        basis.append([(x-k[p])/(k[n]-k[p]) for x in k[1:-1]])
    points=[[F(x) for x in c['point']] for c in s['control_points']]
    weights=[F(c['weight']) for c in s['control_points']]
    if any(len(p)!=3 for p in points) or any(w<=0 for w in weights):
        raise ValueError('positive finite rational weights required')
    return count,degree,basis,points,weights


def continuous_bound(a,b):
    """Return an upper bound in model units, or fail for incompatible bases."""
    ac,ad,ak,ap,aw=_layout(a);bc,bd,bk,bp,bw=_layout(b)
    candidates=[]
    for transpose in (False,True):
        axes=[1,0] if transpose else [0,1]
        if ac!=[bc[i] for i in axes] or ad!=[bd[i] for i in axes]:continue
        for reverse_u in (False,True):
            for reverse_v in (False,True):
                reversals=[reverse_u,reverse_v]
                knots=[([1-x for x in reversed(bk[i])] if reverse else bk[i]) for i,reverse in zip(axes,reversals)]
                if knots!=ak:continue
                qp=[];qw=[]
                for v in range(ac[1]):
                    for u in range(ac[0]):
                        uv=[ac[0]-1-u if reverse_u else u,ac[1]-1-v if reverse_v else v]
                        ij=[uv[1],uv[0]] if transpose else uv
                        index=ij[0]+bc[0]*ij[1];qp.append(bp[index]);qw.append(bw[index])
                d=max(sum((x-y)**2 for x,y in zip(p,q)) for p,q in zip(ap,qp))
                radius=max(sum((x-y)**2 for x,y in zip(q,qp[0])) for q in qp)
                ratios=[v/w for v,w in zip(qw,aw)]
                bound=_sqrt_up(d)+(max(ratios)-min(ratios))/min(ratios)*_sqrt_up(radius)
                out=float(bound)
                while F(out)<bound:out=math.nextafter(out,math.inf)
                candidates.append(out)
    if not candidates:raise ValueError('no matching tensor basis under chart transposition/reversal')
    return min(candidates)
