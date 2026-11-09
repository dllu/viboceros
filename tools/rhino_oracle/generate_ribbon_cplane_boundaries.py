"""Parallel/linear recognition boundaries and nonparallel fit tolerances."""
import copy
from .capture_family import capture
from .generate_ribbon import curve
from .generate_ribbon_cplane import request as base


def request():
    source=base();operations=[]
    flat=next(o for o in source['operations']if o['id']=='cubic')
    tilted=next(o for o in source['operations']if o['id']=='cubic-tilted')
    def add(name,template=flat,**options):
        op=copy.deepcopy(template);op['id']=name;op.update(options);operations.append(op);return op
    for epsilon in [1e-3,1e-5,1e-7,1e-9,1e-11]:
        op=add('parallel-tilt-'+str(epsilon))
        for p in op['curve']['control_points']:p['point'][2]=p['point'][1]*epsilon
    op=add('parallel-height')
    for p in op['curve']['control_points']:p['point'][2]=3.
    for tolerance in [1e-8,1e-4,.01]:add('tilted-tolerance-'+str(tolerance),tilted,fit_tolerance=tolerance)
    for epsilon in [1e-4,1e-6,1e-8]:
        add('linear-projection-'+str(epsilon),curve=curve([[0.,0.,0.],[1.,epsilon,1.],[4.,0.,-1.],[5.,0.,2.]],3))
    op=add('rotated-coplanar',next(o for o in source['operations']if o['id']=='cubic-rotated-cplane'))
    frame=op['cplane']
    for p,original in zip(op['curve']['control_points'],flat['curve']['control_points']):
        x,y,_=original['point'];p['point']=[frame[0][a]+x*frame[1][a]+y*frame[2][a]for a in range(3)]
    for corner in ['Sharp','Round']:
        add('parallel-polyline-'+corner.lower(),curve=curve([[0.,0.,0.],[5.,0.,0.],[5.,4.,0.]]),corner=corner)
    source['operations']=operations;return source


if __name__=='__main__':
    capture('ribbon_cplane_boundaries',request(),['generate_ribbon_cplane_boundaries.py','generate_ribbon_cplane.py',
        'generate_ribbon_smooth.py','generate_ribbon.py','ribbon_probe.py','join_probe.py','merge_edges_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],'VibocerosOracleRibbonCPlaneBoundaries20261109',timeout=1800)
