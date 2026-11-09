"""Independent ordinary Ribbon analytic clocks and closed smooth holdouts."""
import math
from .capture_family import capture
from .generate_ribbon import curve, request as originals


def elevate(c):
    p=c['degree']; points=c['control_points'];h=[[v*w['weight']for v in w['point']]+[w['weight']]for w in points]
    out=[h[0]]
    for i in range(1,p+1):
        a=i/float(p+1);out.append([a*x+(1.-a)*y for x,y in zip(h[i-1],h[i])])
    out.append(h[-1])
    return curve([[x/w,y/w,z/w]for x,y,z,w in out],p+1,[v[3]for v in out])


def request():
    quarter=curve([[3.,0.,0.],[3.,3.,0.],[0.,3.,0.]],2,[1.,math.sqrt(.5),1.])
    cases=[('arc-projective',curve([[3.,0.,0.],[3.,3.,0.],[0.,3.,0.]],2,[1.,2.*math.sqrt(.5),4.])),
        ('arc-projective-gauge',curve([[3.,0.,0.],[3.,3.,0.],[0.,3.,0.]],2,[2.,4.*math.sqrt(.5),8.])),
        ('arc-elevated',elevate(quarter)),
        ('line-cubic-clock',curve([[0.,0.,0.],[.2,0.,0.],[4.,0.,0.],[5.,0.,0.]],3)),
        ('line-rational-clock',curve([[0.,0.,0.],[.2,0.,0.],[4.,0.,0.],[5.,0.,0.]],3,[2.,.7,1.3,3.]))]
    angle=math.radians(37.);sweep=math.radians(120.);center=[2.,-1.,3.];r=4.;w=math.cos(sweep/2.)
    points=[[center[0]+radius*math.cos(a),center[1]+radius*math.sin(a),center[2]]
        for a,radius in [(angle,r),(angle+sweep/2.,r/w),(angle+sweep,r)]]
    cases.append(('arc-120-translated',curve(points,2,[1.,w,1.])))
    circle=next(o['curve']for o in originals()['operations']if o['id']=='circle-outside')
    ellipse=curve([[p['point'][0]*4./3.,p['point'][1]*2./3.,0.]for p in circle['control_points']],
        2,[p['weight']for p in circle['control_points']],circle['knots'])
    cases.append(('ellipse',ellipse))
    cases.append(('closed-cubic',curve([[0.,0.,0.],[3.,0.,0.],[3.,3.,0.],[-3.,3.,0.],[-3.,0.,0.],[0.,0.,0.]],3)))
    operations=[]
    for name,shape in cases:
        operations.append(dict(op='ribbon_command',id=name,curve=shape,distance=.15,side=[0.,5.,0.],
            both_sides=True,corner='Sharp',loose=False,in_cplane=False,output_layer='Current'))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('ribbon_analytic',request(),['generate_ribbon_analytic.py','generate_ribbon.py','ribbon_probe.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonAnalytic20261109',timeout=1200)
