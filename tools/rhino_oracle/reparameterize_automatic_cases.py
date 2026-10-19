"""Independent source definitions for actual Automatic surface commands."""
import copy
import json
import math
from pathlib import Path


def request(artifact_root):
    capture=json.loads(Path(__file__).with_name('observations').joinpath('reparameterize_faces.json').read_text())
    def saved(name):
        row=next(r for r in capture['results'] if r['id']==name+'_automatic')
        d=row['value']['before']['geometry']['geometry']['faces'][0]['definition']
        return dict(degree_u=d['degree'][0],degree_v=d['degree'][1],
            control_point_count_u=d['control_count'][0],control_point_count_v=d['control_count'][1],
            control_points=copy.deepcopy(d['control_points']),knots_u=d['knots_u'][:],knots_v=d['knots_v'][:])
    cases=[]
    def add(name,s):cases.append((name,copy.deepcopy(s)))
    def net(points,weights=None,degree=2):
        return dict(degree_u=degree,degree_v=degree,control_point_count_u=degree+1,control_point_count_v=degree+1,
            control_points=[dict(point=p,weight=w)for p,w in zip(points,weights or [1.]*len(points))],
            knots_u=[0.]*(degree+1)+[1.]*(degree+1),knots_v=[0.]*(degree+1)+[1.]*(degree+1))
    add('bilinear_warp',net([[0.,0.,0.],[4.,0.,0.],[0.,3.,0.],[8.,3.,4.]],degree=1))
    add('rational_bilinear',net([[0.,0.,0.],[3.,4.,0.],[0.,0.,12.],[3.,4.,12.]],[.25,2.,3.,.5],1))
    points=[[2.*i,1.5*j,6. if (i,j)==(1,1) else .3*i*j]for j in range(3)for i in range(3)]
    wave=net(points);add('wave',wave)
    add('weighted_wave',net(points,[1.,.5,2.,1.5,3.,.75,.9,1.1,1.]))
    source=json.loads(Path(__file__).with_name('fixtures').joinpath('reparameterize.json').read_text())
    unclamped=next(o for o in source['operations']if o['id']=='rational-nonclamped-surface')
    add('unclamped',{k:copy.deepcopy(unclamped[k])for k in ['degree_u','degree_v','control_point_count_u','control_point_count_v','control_points','knots_u','knots_v']})
    sphere=saved('sphere');add('sphere',sphere)
    transposed=copy.deepcopy(sphere);nu,nv=sphere['control_point_count_u'],sphere['control_point_count_v']
    transposed['control_points']=[copy.deepcopy(sphere['control_points'][u*nu+v])for v in range(nu)for u in range(nv)]
    for a,b in [('degree_u','degree_v'),('control_point_count_u','control_point_count_v'),('knots_u','knots_v')]:transposed[a],transposed[b]=transposed[b],transposed[a]
    add('sphere_transposed',transposed)
    for name,domains in [('sphere_affine',[[-2.,8.],[10.,30.]]),('sphere_anisotropic',[[2e-8,7e-8],[-8000.,-3000.]])]:
        s=copy.deepcopy(sphere)
        for axis,d in zip(['u','v'],domains):
            k=s['knots_'+axis];a,b=k[s['degree_'+axis]],k[s['control_point_count_'+axis]]
            s['knots_'+axis]=[d[0]+(d[1]-d[0])*((t-a)/(b-a))for t in k]
        add(name,s)
    for scale in [.001,1000.]:
        s=copy.deepcopy(sphere)
        for cp in s['control_points']:cp['point']=[x*scale for x in cp['point']]
        add('sphere_scale_'+('small' if scale<1 else 'large'),s)
    s=copy.deepcopy(sphere)
    for cp in s['control_points']:cp['point']=[x+t for x,t in zip(cp['point'],[10000.,-5000.,3000.])]
    add('sphere_translated',s)
    s=copy.deepcopy(sphere)
    for cp in s['control_points']:cp['weight']*=1e20
    add('sphere_weight_gauge',s)
    cylinder=saved('cylinder');add('cylinder',cylinder)
    cone=copy.deepcopy(cylinder)
    for i,cp in enumerate(cone['control_points']):
        if i>=cone['control_point_count_u']:cp['point']=[cp['point'][0]*.25,cp['point'][1]*.25,4.]
    add('cone',cone)
    quarter=dict(degree_u=2,degree_v=1,control_point_count_u=3,control_point_count_v=2,
        knots_u=[0.,0.,0.,1.,1.,1.],knots_v=[0.,0.,1.,1.],
        control_points=[dict(point=[x,y,z],weight=w)for z in [-1.,3.]for x,y,w in [(2.,0.,1.),(2.,2.,math.sqrt(.5)),(0.,2.,1.)]])
    add('quarter',quarter)
    for k in [.5,2.]:
        s=copy.deepcopy(quarter)
        for row in range(2):
            for i in range(3):s['control_points'][row*3+i]['weight']*=k**i
        add('mobius_'+str(int(k*2)),s)
    def h(cp):return [x*cp['weight']for x in cp['point']]+[cp['weight']]
    def cp(v):return dict(point=[x/v[3]for x in v[:3]],weight=v[3])
    def mix(a,b,t):return [(1-t)*x+t*y for x,y in zip(a,b)]
    for name in ['elevated','split']:
        s=copy.deepcopy(quarter);controls=[]
        for row in range(2):
            q=list(map(h,s['control_points'][row*3:row*3+3]))
            if name=='elevated':q=[q[0],mix(q[1],q[0],1/3),mix(q[2],q[1],2/3),q[2]]
            else:
                left=mix(q[0],q[1],.3);right=mix(q[1],q[2],.3);q=[q[0],left,mix(left,right,.3),right,q[2]]
            controls.extend(map(cp,q))
        s['control_points']=controls;s['control_point_count_u']=len(controls)//2
        if name=='elevated':s['degree_u']=3;s['knots_u']=[0.]*4+[1.]*4
        else:s['knots_u']=[0.]*3+[.3]*2+[1.]*3
        add(name,s)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),
        operations=[dict(op='reparameterize_face_command',id=name+'_automatic',family='nurbs',domains='Automatic',surface=s,
            artifact_path=artifact_root.rstrip('/')+'/'+name+'_automatic.3dm')for name,s in cases])
