"""Resolve plane switching independently of distance and coordinate scale."""
import copy
from .capture_family import capture
from .generate_ribbon import curve
from .generate_ribbon_cplane import request as base


def request():
    source=base();flat=next(o for o in source['operations']if o['id']=='cubic');operations=[]
    def add(name,**opts):
        op=copy.deepcopy(flat);op['id']=name;op.update(opts);operations.append(op);return op
    for epsilon in [.006,.007,.008,.0087,.0088,.009]:
        op=add('angle-'+str(epsilon))
        for p in op['curve']['control_points']:p['point'][2]=p['point'][1]*epsilon
    for scale in [.1,10.]:
        op=add('scale-'+str(scale),distance=.15*scale,side=[0.,5.*scale,0.])
        for p in op['curve']['control_points']:
            x,y,_=p['point'];p['point']=[x*scale,y*scale,y*.008*scale]
    for d in [.01,.5]:
        op=add('distance-'+str(d),distance=d)
        for p in op['curve']['control_points']:p['point'][2]=p['point'][1]*.008
    for epsilon in [.005,.006,.0062,.007,.008]:
        add('perpendicular-'+str(epsilon),curve=curve([[0.,0.,0.],[1.,epsilon,1.],[4.,0.,-1.],[5.,0.,2.]],3))
    for h in [1e-4,.01]:
        op=add('spatial-near-plane-'+str(h))
        for p,z in zip(op['curve']['control_points'],[0.,h,-h,2.*h]):p['point'][2]=z
    source['operations']=operations;return source


if __name__=='__main__':
    capture('ribbon_cplane_switches',request(),['generate_ribbon_cplane_switches.py','generate_ribbon_cplane.py',
        'generate_ribbon_smooth.py','generate_ribbon.py','ribbon_probe.py','join_probe.py','merge_edges_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],'VibocerosOracleRibbonCPlaneSwitches20261109',timeout=1800)
