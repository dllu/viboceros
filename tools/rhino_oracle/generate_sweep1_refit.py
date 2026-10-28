"""Refit profiles through the public Sweep1 SDK before drawing implementation conclusions."""
import copy
from .capture_family import capture
from .generate_sweep1_rebuild import request as rebuilt


def request(command=False):
    root=rebuilt(command)
    base={op['id']:op for op in root['operations']}
    operations=[]
    for name in ['open-5','closed-5','rational-5','polyline-5','two-open-5','two-closed-5']:
        tolerances=[0.,1e-9,1e-6,.001,.01,.1]if name in ['open-5','closed-5']else[.001,.01]
        for tolerance in tolerances:
            op=copy.deepcopy(base[name]);op.pop('rebuild_point_count')
            op['id']=name.replace('-5','')+'-refit-'+str(tolerance)
            op['refit_tolerance']=tolerance
            operations.append(op)
    root['operations']=operations
    return root


if __name__=='__main__':
    capture('sweep1_refit_sdk',request(),[
        'generate_sweep1_refit.py','generate_sweep1_rebuild.py','sweep1_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepRefitSDK20261027',timeout=300)
