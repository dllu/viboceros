"""Observe angular switching, coarse fits and near-perpendicular control planes."""
import copy
from .capture_family import capture
from .generate_ribbon import curve
from .generate_ribbon_cplane import request as base


def request():
    source=base();flat=next(o for o in source['operations']if o['id']=='cubic');operations=[]
    def add(name,**opts):
        op=copy.deepcopy(flat);op['id']=name;op.update(opts);operations.append(op);return op
    for epsilon in [.005,.01,.017,.018,.025,.1]:
        op=add('parallel-angle-'+str(epsilon))
        for p in op['curve']['control_points']:p['point'][2]=p['point'][1]*epsilon
    for angle in [1e-6,.1]:
        op=add('angle-setting-'+str(angle),model_angle_tolerance=angle)
        for p in op['curve']['control_points']:p['point'][2]=p['point'][1]*.025
    for epsilon in [.001,.01,.1]:
        add('perpendicular-'+str(epsilon),curve=curve([[0.,0.,0.],[1.,epsilon,1.],[4.,0.,-1.],[5.,0.,2.]],3))
    for tolerance in [.001,.01,.1]:
        for d in [.15,.5]:
            op=add('fit-'+str(tolerance)+'-distance-'+str(d),fit_tolerance=tolerance,distance=d)
            for p in op['curve']['control_points']:p['point'][2]=p['point'][1]
    for y in [.0001,1.,10.]:
        op=add('height-'+str(y))
        for p in op['curve']['control_points']:p['point'][2]=p['point'][1]*y
    source['operations']=operations;return source


if __name__=='__main__':
    capture('ribbon_cplane_policy',request(),['generate_ribbon_cplane_policy.py','generate_ribbon_cplane.py',
        'generate_ribbon_smooth.py','generate_ribbon.py','ribbon_probe.py','join_probe.py','merge_edges_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],'VibocerosOracleRibbonCPlanePolicy20261109',timeout=1800)
