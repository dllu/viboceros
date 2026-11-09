"""Public SDK achieved tolerance for the ambiguous curved fitting cases."""
import copy
from .capture_family import capture
from .generate_surface_fit_strips import request as strips
from .generate_surface_fit_adaptive import request as adaptive
from .generate_surface_fit_capacity_holdouts import request as holdouts


def request():
    ids=['fit-strip-rational_bezier-0-3-0.01','fit-strip-rational_bezier-0-3-0.001',
         'fit-adaptive-polynomial-1-3-0.05','fit-adaptive-polynomial-1-3-0.01',
         'fit-adaptive-rational_bezier-0-3-0.01','fit-adaptive-rational_bezier-1-3-0.05',
         'fit-adaptive-rational_bezier-1-3-0.01','fit-adaptive-rational_bezier-1-3-0.001',
         'fit-capacity-holdout-2-1-distinct','fit-capacity-holdout-4-3-distinct']
    all_ops=[o for module in [strips,adaptive,holdouts]for o in module()['operations']]
    operations=[]
    for ident in ids:
        op=copy.deepcopy(next(o for o in all_ops if o['id']==ident));op['op']='surface_fit_achieved';op['id']='achieved-'+ident
        operations.append(op)
    original=copy.deepcopy(operations[0])
    for limit in [.002,.005,.02,.03,.05]:
        op=copy.deepcopy(original);op['fit_tolerance']=limit;op['id']='achieved-strip-sweep-'+str(limit);operations.append(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('surface_fit_achieved',request(),['generate_surface_fit_achieved.py','surface_fit_achieved_probe.py',
        'surface_fit_probe.py','generate_surface_fit_capacity_holdouts.py','generate_surface_fit_capacity.py',
        'generate_surface_fit_strips.py','generate_surface_fit_adaptive.py','generate_surface_knot_reduction.py',
        'generate_surface_knot_reduction_holdouts.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSurfaceFitAchieved20261109',timeout=1200)
