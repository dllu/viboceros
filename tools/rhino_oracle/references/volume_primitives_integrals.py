"""Exact polynomial witnesses for two divergence-theorem moment conventions.

Only source geometry is read. Arithmetic is Fraction on the stored binary64
coordinates; no native integrator or Rhino observation is used. Bilinear
surfaces are integrated symbolically over [0,1]^2, meshes as exact tetrahedra.
"""
from fractions import Fraction as F
import json
from .volume_surface_primitives import request


class Polynomial:
    def __init__(self, terms): self.terms = {k:F(v) for k,v in terms.items() if v}
    @staticmethod
    def constant(value): return Polynomial({(0,0):value})
    def __add__(self, other):
        other = other if isinstance(other,Polynomial) else self.constant(other)
        terms = dict(self.terms)
        for k,v in other.terms.items(): terms[k] = terms.get(k,F(0)) + v
        return Polynomial(terms)
    __radd__ = __add__
    def __neg__(self): return self * -1
    def __sub__(self, other): return self + -other
    def __mul__(self, other):
        other = other if isinstance(other,Polynomial) else self.constant(other)
        terms = {}
        for a,x in self.terms.items():
            for b,y in other.terms.items():
                k = (a[0]+b[0],a[1]+b[1])
                terms[k] = terms.get(k,F(0)) + x*y
        return Polynomial(terms)
    __rmul__ = __mul__
    def derivative(self, axis):
        return Polynomial({(k[0]-(axis==0),k[1]-(axis==1)): v*k[axis]
                           for k,v in self.terms.items() if k[axis]})
    def integral(self): return sum((v/F((i+1)*(j+1)) for (i,j),v in self.terms.items()),F(0))


def cross(a,b): return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
def dot(a,b): return sum((x*y for x,y in zip(a,b)),0)


def surface_integrals(source, base, convention):
    assert source['degree_u']==source['degree_v']==1
    assert source['control_point_count_u']==source['control_point_count_v']==2
    assert all(c['weight']==1 for c in source['control_points'])
    points = [list(map(F,c['point'])) for c in source['control_points']]
    q = [Polynomial({(0,0):points[0][i]-base[i], (1,0):points[1][i]-points[0][i],
                     (0,1):points[2][i]-points[0][i],
                     (1,1):points[3][i]-points[2][i]-points[1][i]+points[0][i]}) for i in range(3)]
    n = cross([p.derivative(0) for p in q],[p.derivative(1) for p in q])
    flux = dot(q,n)
    volume = flux.integral()/3
    if convention=='cone': local = [(p*flux).integral()/4 for p in q]
    elif convention=='coordinate':
        local = [(q[i]*flux - q[i]*q[i]*n[i]*F(1,2)).integral()/3 for i in range(3)]
    else: raise ValueError('unknown moment convention')
    return volume,[base[i]*volume+local[i] for i in range(3)]


def mesh_integrals(source, base):
    points = [list(map(F,p)) for p in source['vertices']]
    triangles = []
    for f in source['faces']:
        if len(f)==3: triangles.append(f)
        elif len(f)==4:
            distance = lambda i,j: sum((x-y)**2 for x,y in zip(points[f[i]],points[f[j]]))
            triangles.extend([[f[i] for i in t] for t in
                ([(0,1,2),(0,2,3)] if distance(0,2)<=distance(1,3) else [(0,1,3),(1,2,3)])])
        else: raise ValueError('unsupported mesh face')
    volume = F(0); first = [F(0)]*3
    for indices in triangles:
        p = [points[i] for i in indices]
        q = [[a-b for a,b in zip(point,base)] for point in p]
        v = dot(q[0],cross(q[1],q[2]))/6
        volume += v
        first = [first[i]+v*(base[i]+sum(point[i] for point in p))/4 for i in range(3)]
    return volume,first


def collection(sources, convention):
    all_points = [p for source in sources for p in
        ([c['point'] for c in source['control_points']] if source['type']=='surface' else
         [source['vertices'][i] for f in source['faces'] for i in f])]
    # The shared kernel reference is a binary64 point, not an unrounded rational
    # midpoint. Keep that one intentional reference rounding explicit.
    base = [F(float((min(F(p[i]) for p in all_points)+max(F(p[i]) for p in all_points))/2)) for i in range(3)]
    volume = F(0); first = [F(0)]*3
    for source in sources:
        if source['type']=='surface': v,m = surface_integrals(source,base,convention)
        elif source['type']=='mesh': v,m = mesh_integrals(source,base)
        else: raise ValueError('unsupported reference geometry')
        volume += v; first = [a+b for a,b in zip(first,m)]
    centroid = None if not volume else [x/volume for x in first]
    return dict(base=[str(x) for x in base], volume=str(volume), first=[str(x) for x in first],
                centroid=None if centroid is None else [str(x) for x in centroid],
                rounded_centroid=None if centroid is None else list(map(float,centroid)),rounded_volume=float(volume))


def reference():
    return dict(formulas=dict(volume='integral(q dot n)/3', cone='integral(q_j * (q dot n))/4',
        coordinate='integral(q_j * (q dot n) - q_j^2*n_j/2)/3'),
        cases={op['id']:{mode:collection(op['sources'],mode) for mode in ('cone','coordinate')}
               for op in request()['operations']})


if __name__=='__main__': print(json.dumps(reference(),indent=2,allow_nan=False))
