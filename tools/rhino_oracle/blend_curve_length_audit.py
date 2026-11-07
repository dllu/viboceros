"""Independent expected-data quadrature for the captured G3 arc domain drift."""
import math

def measure(curve):
    points = [p['point'] for p in curve['control_points']]
    n = curve['degree']
    if len(points) != n+1 or any(p['weight'] != 1 for p in curve['control_points']):
        raise ValueError('expected a polynomial Bezier capture')
    net = [[n*(b-a) for a,b in zip(p,q)] for p,q in zip(points,points[1:])]
    def speed(t):
        rows = net
        while len(rows) > 1:
            rows = [[(1-t)*a+t*b for a,b in zip(p,q)] for p,q in zip(rows,rows[1:])]
        return math.hypot(*rows[0])
    def integrate(a,b,fa,fm,fb,whole,budget,depth):
        m = (a+b)/2
        l,r = speed((a+m)/2),speed((m+b)/2)
        left,right = (m-a)*(fa+4*l+fm)/6,(b-m)*(fm+4*r+fb)/6
        delta = left+right-whole
        if abs(delta) <= 15*budget: return left+right+delta/15
        if not depth: raise ValueError('independent length audit did not converge')
        return (integrate(a,m,fa,l,fm,left,budget/2,depth-1)
                + integrate(m,b,fm,r,fb,right,budget/2,depth-1))
    fa,fm,fb = speed(0),speed(.5),speed(1)
    length = integrate(0,1,fa,fm,fb,(fa+4*fm+fb)/6,1e-11,24)
    native = curve['domain'][1]-curve['domain'][0]
    return dict(native_domain_span=native,independent_length=length,
                native_domain_minus_length=native-length,
                method='Adaptive Simpson integration of the captured polynomial derivative; absolute budget 1e-11. Expected data only.')
