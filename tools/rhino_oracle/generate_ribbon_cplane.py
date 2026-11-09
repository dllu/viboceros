"""Fixed-plane Ribbon fitting across degree, weights, charts and view planes."""
import copy
from .capture_family import capture
from .generate_ribbon import curve
from .generate_ribbon_smooth import request as smooth


def request():
    shapes={o['id'].rsplit('-',1)[0]:o['curve']for o in smooth()['operations']if o['id'].endswith('-left')}
    shapes['closed']=curve([[0.,0.,0.],[3.,0.,0.],[3.,3.,0.],[-3.,3.,0.],[-3.,0.,0.],[0.,0.,0.]],3)
    shapes['line-clock']=curve([[0.,0.,0.],[.2,0.,1.],[4.,0.,-1.],[5.,0.,2.]],3,[2.,.7,1.3,3.])
    shapes['degree4']=curve([[0.,0.,0.],[1.,2.,1.],[2.,3.,-1.],[4.,-1.,1.],[5.,1.,0.]],4)
    shapes['near-line']=curve([[0.,0.,0.],[1.,1e-7,1.],[4.,0.,-1.],[5.,0.,2.]],3)
    operations=[]
    def add(name,c,**opts):
        op=dict(op='ribbon_command',id=name,curve=copy.deepcopy(c),distance=.15,side=[0.,5.,0.],both_sides=False,
            corner='Sharp',loose=False,in_cplane=True,output_layer='Current',fit_tolerance=1e-6)
        op.update(opts);operations.append(op)
    for name,c in shapes.items():
        add(name,c)
        if name in ['cubic','rational','quadratic','nonuniform','circle','arc','closed']:
            tilted=copy.deepcopy(c)
            for p in tilted['control_points']:p['point'][2]=p['point'][1]
            add(name+'-tilted',tilted,both_sides=True)
    for d in [.0001,.01,.5]:add('cubic-distance-'+str(d),shapes['cubic'],distance=d)
    for t in [0.,1e-8,1e-4,.01]:add('cubic-tolerance-'+str(t),shapes['cubic'],fit_tolerance=t)
    for scale in [.01,100.]:
        c=copy.deepcopy(shapes['cubic'])
        for p in c['control_points']:p['point']=[x*scale for x in p['point']]
        add('cubic-scale-'+str(scale),c,distance=.15*scale,side=[0.,5.*scale,0.])
    c=copy.deepcopy(shapes['rational']);c['domain']=[-4.,8.]
    add('rational-chart',c,both_sides=True)
    for gauge in [.5,2.,10.]:
        c=copy.deepcopy(shapes['rational'])
        for p in c['control_points']:p['weight']*=gauge
        add('rational-gauge-'+str(gauge),c,both_sides=True)
    frame=[[2.,3.,4.],[0.,.6,.8],[1.,0.,0.]];normal=[0.,.8,-.6]
    for name in ['cubic','rational']:
        c=copy.deepcopy(shapes[name])
        for i,p in enumerate(c['control_points']):
            x,y,_=p['point'];h=[0.,1.,-1.,2.][i]
            p['point']=[frame[0][a]+x*frame[1][a]+y*frame[2][a]+h*normal[a]for a in range(3)]
        side=[frame[0][a]+5.*frame[2][a]for a in range(3)]
        add(name+'-rotated-cplane',c,cplane=frame,side=side,both_sides=True)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('ribbon_cplane',request(),['generate_ribbon_cplane.py','generate_ribbon_smooth.py','generate_ribbon.py','ribbon_probe.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonCPlane20261109',timeout=1800)
