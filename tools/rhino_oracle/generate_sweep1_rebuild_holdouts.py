"""Rebuilt sections composed with point poles and a retained curved rail."""
import copy
import math
from .capture_family import capture
from .generate_sweep1_rebuild import request


def query():
    root=request(command=True)
    operations=[]
    for name in ['open-5','closed-5']:
        op=copy.deepcopy(next(op for op in root['operations'] if op['id']==name))
        op['id']=name+'-end';op['end_point']=[0.,0.,5.]
        operations.append(op)
    op=copy.deepcopy(operations[1]);op['id']='closed-5-both'
    op['start_point']=[0.,0.,0.];op['sections'][0]['center'][2]=2.5;op['parameters']=[2.5]
    operations.append(op)
    op=copy.deepcopy(operations[0]);op.pop('end_point');op['id']='arc-open-5'
    op['rail']=dict(type='arc',points=[[0.,0.,0.],[3./math.sqrt(2.),0.,3.-3./math.sqrt(2.)],[3.,0.,3.]])
    op['refit_rail']=False
    operations.append(op)
    root['operations']=operations
    return root


if __name__=='__main__':
    capture('sweep1_rebuild_holdouts',query(),[
        'generate_sweep1_rebuild_holdouts.py','generate_sweep1_rebuild.py','sweep1_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepRebuildHoldouts20261027',timeout=240)
