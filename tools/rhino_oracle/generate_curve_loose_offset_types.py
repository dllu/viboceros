"""Holdouts for analytic runtime types, piecewise curves and unclamped bases."""
import copy
from .capture_family import capture
from .generate_curve_loose_offset import request as original
from .generate_ribbon import curve


def request():
    root=original();by_id={o['id']:o for o in root['operations']};ops=[]
    for name,base,kind in [('native-line','loose-line','line'),('native-circle','loose-circle-outside','circle'),
                           ('native-polyline','loose-corner-sharp','polyline'),('native-polycurve','loose-corner-sharp','polycurve'),
                           ('native-polycurve-both','loose-corner-sharp','polycurve'),('native-periodic','loose-smooth','periodic')]:
        op=copy.deepcopy(by_id[base]);op.update(id=name,native_kind=kind)
        if name.endswith('-both'):op['both_sides']=True
        ops.append(op)
    for name,shape in [
        ('degree-four',curve([[0.,0.,0.],[1.,2.,0.],[2.,1.,0.],[3.,-1.,0.],[5.,1.,0.]],4)),
        ('unclamped',curve([[0.,0.,0.],[1.,2.,0.],[3.,3.,0.],[5.,1.,0.]],2,knots=[-2.,-1.,0.,1.,2.,3.,4.])),
        ('rational-gauge',curve([[0.,0.,0.],[1.,2.,0.],[4.,-1.,0.],[5.,1.,0.]],3,[2.,1.4,2.6,2.]))]:
        op=copy.deepcopy(by_id['loose-smooth']);op.update(id=name,curve=shape);ops.append(op)
    root['operations']=ops;return root


if __name__=='__main__':
    capture('curve_loose_offset_types',request(),['generate_curve_loose_offset_types.py','generate_curve_loose_offset.py',
        'generate_ribbon.py','ribbon_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleCurveLooseOffsetTypes20261109',timeout=1200)
