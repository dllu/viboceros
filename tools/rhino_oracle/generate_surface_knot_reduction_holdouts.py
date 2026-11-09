"""Redundant curved tensors, including rational and unequal-degree bases."""
import copy
from .capture_family import capture


def insert_line(points,knots,degree,t):
    n=len(points);k=max(i for i in range(degree,n)if knots[i]<=t);s=knots.count(t)
    h=[[p['weight']*x for x in p['point']]+[p['weight']]for p in points]
    q=[None]*(n+1)
    for i in range(k-degree+1):q[i]=h[i]
    for i in range(k-s+1,n+1):q[i]=h[i-1]
    for i in range(k-degree+1,k-s+1):
        a=(t-knots[i])/(knots[i+degree]-knots[i]);q[i]=[(1-a)*x+a*y for x,y in zip(h[i-1],h[i])]
    return [dict(point=[x/p[3]for x in p[:3]],weight=p[3])for p in q],knots[:k+1]+[t]+knots[k+1:]


def surface(rational,q):
    controls=[]
    for j in range(q+1):
        for i in range(4):
            z=.3*i*(i-1)/6.+.2*j*(j-1)/(q*(q-1))+.15*i*j/(3.*q)
            weight=[1.,.75,1.5,1.25][i]*[1.,2.,.5,1.][j]if rational else 1.
            controls.append(dict(point=[i/3.,j/float(q),z],weight=weight))
    ku=[0.]*4+[1.]*4;kv=[0.]*(q+1)+[1.]*(q+1);nu=4;nv=q+1
    for t in [.25,.25,.5,.5,.5]:
        rows=[]
        for j in range(nv):row,following=insert_line(controls[j*nu:(j+1)*nu],ku,3,t);rows+=row
        controls=rows;ku=following;nu+=1
    for t in [.5,.5]:
        cols=[]
        for i in range(nu):col,following=insert_line([controls[j*nu+i]for j in range(nv)],kv,q,t);cols.append(col)
        nv+=1;controls=[cols[i][j]for j in range(nv)for i in range(nu)];kv=following
    return dict(degree_u=3,degree_v=q,control_point_count_u=nu,control_point_count_v=nv,
        knots_u=ku,knots_v=kv,control_points=controls,domain_u=[-4.,2.],domain_v=[3.,11.])


def request():
    operations=[]
    for rational in [False,True]:
        for q in [2,3]:
            for direction in ['u','v','both']:
                for limit in [0.,1e-9,.001]:
                    operations.append(dict(op='surface_knot_reduction',id='surface-reduce-redundant-'+str(int(rational))+'-'+str(q)+'-'+direction+'-'+str(limit),
                        surface=copy.deepcopy(surface(rational,q)),direction=direction,minimum_multiplicity=1,maximum_multiplicity=4,absolute_tolerance=limit))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('surface_knot_reduction_holdouts',request(),['generate_surface_knot_reduction_holdouts.py','surface_knot_reduction_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],'VibocerosOracleSurfaceKnotReductionHoldouts20261108',timeout=1200)
