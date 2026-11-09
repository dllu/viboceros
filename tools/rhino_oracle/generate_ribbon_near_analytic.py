"""Recognition boundaries and reversed analytic traversal for ordinary Ribbon."""
import math
from .capture_family import capture
from .generate_ribbon import curve


def request():
    cases=[]
    for epsilon in [1e-4,1e-6,1e-8,1e-10]:
        cases.append(('near-circle-'+str(epsilon),curve([[3.,0.,0.],[3.,3.,0.],[0.,3.,0.]],2,
            [1.,math.sqrt(.5)+epsilon,1.])))
    for epsilon in [1e-4,1e-7,1e-10]:
        cases.append(('near-line-'+str(epsilon),curve([[0.,0.,0.],[.2,epsilon,0.],[4.,0.,0.],[5.,0.,0.]],3)))
    cases.append(('reversed-arc',curve([[0.,3.,0.],[3.,3.,0.],[3.,0.,0.]],2,[1.,math.sqrt(.5),1.])))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='ribbon_command',id=name,curve=shape,distance=.15,side=[0.,5.,0.],both_sides=True,
            corner='Sharp',loose=False,in_cplane=False,output_layer='Current')for name,shape in cases])


if __name__=='__main__':
    capture('ribbon_near_analytic',request(),['generate_ribbon_near_analytic.py','generate_ribbon.py','ribbon_probe.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonNearAnalytic20261109',timeout=1200)
