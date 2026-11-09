"""Ruled Ribbon holdouts: loose profiles, rational gauges and source charts."""
import copy
from .capture_family import capture
from .generate_curve_loose_offset import request as offsets
from .generate_curve_loose_offset_types import request as types


def request():
    root=copy.deepcopy(offsets())
    root['operations']+=copy.deepcopy(types()['operations'])
    for op in root['operations']:
        op['op']='ribbon_command';op['id']='ribbon-'+op['id']
    original=copy.deepcopy(next(o for o in root['operations']if o['id']=='ribbon-loose-rational'))
    for gauge in [0.5,2.,10.]:
        op=copy.deepcopy(original);op['id']='ribbon-rational-gauge-'+str(gauge)
        for p in op['curve']['control_points']:p['weight']*=gauge
        root['operations'].append(op)
    for name in ['ribbon-loose-line','ribbon-loose-smooth','ribbon-loose-circle-outside']:
        op=copy.deepcopy(next(o for o in root['operations']if o['id']==name))
        op['id']=name+'-chart';op['curve']['domain']=[-4.,8.];root['operations'].append(op)
    return root


if __name__=='__main__':
    capture('ribbon_ruled',request(),['generate_ribbon_ruled.py','generate_curve_loose_offset.py',
        'generate_curve_loose_offset_types.py','generate_ribbon.py','ribbon_probe.py','join_probe.py',
        'merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonRuled20261109',timeout=1200)
