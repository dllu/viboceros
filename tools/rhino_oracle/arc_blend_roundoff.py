"""Independent join formulas evaluated only through public Rhino arc APIs."""
from math import sqrt

def observe(recipe,host,observe_curve):
    G=host['Rhino'].Geometry
    p,q=recipe['start'],recipe['end'];t0,t1=recipe['start_direction'],recipe['end_direction']
    v=[b-a for a,b in zip(p,q)];delta=[a-b for a,b in zip(t0,t1)]
    dot=lambda a,b:sum(x*y for x,y in zip(a,b))
    length=sqrt(dot(v,v));direction=[x/length for x in v]
    target=recipe['search_target']
    def joint(a,kind):
        r=a/(1.-a)
        if kind.startswith('normalized'):
            b=1.-a;A=a*b*dot(delta,delta);B=2.*dot(direction,[a*x+b*y for x,y in zip(t0,t1)])
            d=2./(B+sqrt(B*B+4.*A))if kind.endswith('stable')else(-B+sqrt(B*B+4.*A))/(2.*A)
            return [p[i]+a*v[i]+a*b*d*length*delta[i]for i in range(3)]
        A=2.*r*(1.-dot(t0,t1));B=2.*dot(v,[r*x+y for x,y in zip(t0,t1)]);C=dot(v,v)
        root=sqrt(B*B+4.*A*C)
        d1=2.*C/(B+root)if 'stable'in kind else(-B+root)/(2.*A)
        d0=r*d1;total=d0+d1
        if kind.endswith('qlerp') or kind.endswith('qweight'):
            q0=[p[i]+d0*t0[i]for i in range(3)];q1=[q[i]-d1*t1[i]for i in range(3)]
            if kind.endswith('qlerp'):return [q0[i]+a*(q1[i]-q0[i])for i in range(3)]
            return [(q0[i]+r*q1[i])/(1.+r)for i in range(3)]
        if kind.endswith('weighted'):
            return [(d1*p[i]+d0*q[i]+d0*d1*delta[i])/total for i in range(3)]
        return [p[i]+a*v[i]+d0*d1/total*delta[i]for i in range(3)]
    def value(a,kind):
        if kind=='sdk':
            curve=G.Curve.CreateArcBlend(G.Point3d(*p),G.Vector3d(*t0),G.Point3d(*q),G.Vector3d(*t1),a/(1.-a))
            if curve is None:raise ValueError('SDK search trial failed')
            try:return curve.SegmentCurve(0).Arc.Radius-curve.SegmentCurve(1).Arc.Radius
            finally:curve.Dispose()
        j=G.Point3d(*joint(a,kind))
        first=G.Arc(G.Point3d(*p),G.Vector3d(*t0),j)
        second=G.Arc(G.Point3d(*q),G.Vector3d(*[-x for x in t1]),j)
        if not first.IsValid or not second.IsValid:raise ValueError('independent arc trial failed')
        return first.Radius-second.Radius
    results={}
    for kind in ['sdk','normalized_stable','normalized_naive','dim_stable_weighted','dim_naive_weighted','dim_stable_chord','dim_naive_chord','dim_stable_qlerp','dim_naive_qlerp','dim_stable_qweight','dim_naive_qweight']:
        lo,hi=1./101.,100./101.;lv,hv=value(lo,kind),value(hi,kind)
        best=(lo,lv)if abs(lv-target)<abs(hv-target)else(hi,hv);steps=[]
        for _ in range(64):
            a=.5*(lo+hi)
            if a==lo or a==hi:break
            f=value(a,kind);steps.append(dict(fraction=a,radius_difference=f))
            if abs(f-target)<abs(best[1]-target):best=a,f
            if abs(f-target)<=1./4294967296.:best=a,f;break
            if abs(lv-target)<abs(hv-target):hi,hv=a,f
            else:lo,lv=a,f
        results[kind]=dict(fraction=best[0],radius_difference=best[1],steps=steps)
    a=results['sdk']['fraction']
    curve=G.Curve.CreateArcBlend(G.Point3d(*p),G.Vector3d(*t0),G.Point3d(*q),G.Vector3d(*t1),a/(1.-a))
    try:results['sdk']['curve']=observe_curve(curve)
    finally:curve.Dispose()
    if recipe.get('search_rules'):
        from arc_blend_search_rules import observe
        results['rules']=observe(recipe,host)
    return results
