"""Representation, region matching, and point/curved-rail refit holdouts."""
import copy
import math
import sys
from .capture_family import capture
from .generate_sweep1_refit import request as base_request
from .generate_sweep1_rebuild_holdouts import query as arc_request


def request(command=False):
    root=base_request(command)
    template=next(op for op in root['operations'] if op['id']=='open-refit-0.01')
    ops=[]
    def add(name,sections,tolerance):
        op=copy.deepcopy(template);op.update(id=name,sections=sections,refit_tolerance=tolerance,parameters=[0.]if len(sections)==1 else[0.,5.])
        ops.append(op)
        return op
    if not command:
        points=[[2.,0.,0.],[2.,2.,0.],[0.,2.,0.],[-2.,2.,0.],[-2.,0.,0.],[-2.,-2.,0.],[0.,-2.,0.],[2.,-2.,0.],[2.,0.,0.]]
        circle=dict(type='nurbs',degree=2,knots=[0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.],control_points=[dict(point=p,weight=1. if i%2==0 else math.sqrt(.5))for i,p in enumerate(points)])
        for tolerance in [1e-6,.001]:add('nurbs-circle-'+str(tolerance),[circle],tolerance)
        loop=dict(type='nurbs',degree=3,knots=[0.,0.,0.,0.,1.,1.,1.,1.],control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[2.,2.,0.],[-2.,2.,0.],[0.,0.,0.]]])
        for tolerance in [.001,.01]:add('closed-cubic-'+str(tolerance),[loop],tolerance)
        polyline=next(op for op in root['operations'] if op['id']=='polyline-refit-0.01')['sections'][0]
        second=dict(type='polyline',vertices=[[0.,0.,5.],[1.,2.,5.],[2.,-.3,5.],[3.,1.,5.]])
        for tolerance in [0.,.001,.01]:add('different-corners-'+str(tolerance),[polyline,second],tolerance)
        closed=next(op for op in root['operations'] if op['id']=='closed-refit-0.01')['sections'][0]
        second=copy.deepcopy(template['sections'][0])
        for c in second['control_points']:c['point'][2]=5.
        add('mixed-open-closed',[closed,second],.01)
    else:
        for kind in ['open','closed']:
            op=copy.deepcopy(next(op for op in root['operations'] if op['id']==kind+'-refit-0.01'))
            op['id']=kind+'-end';op['end_point']=[0.,0.,5.];ops.append(op)
        op=copy.deepcopy(ops[1]);op['id']='closed-both';op['start_point']=[0.,0.,0.];op['parameters']=[2.5];op['sections'][0]['center'][2]=2.5;ops.append(op)
        op=copy.deepcopy(next(op for op in arc_request()['operations'] if op['id']=='arc-open-5'));op.pop('rebuild_point_count');op.update(id='retained-arc',refit_tolerance=.01);ops.append(op)
    root['operations']=ops
    return root


if __name__=='__main__':
    command='--commands'in sys.argv
    capture('sweep1_refit_command_holdouts'if command else'sweep1_refit_sdk_holdouts',request(command),[
        'generate_sweep1_refit_holdouts.py','generate_sweep1_refit.py','generate_sweep1_rebuild.py',
        'generate_sweep1_rebuild_holdouts.py','sweep1_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepRefitHoldout'+('Command'if command else'sSDK')+'20261027',timeout=300)
