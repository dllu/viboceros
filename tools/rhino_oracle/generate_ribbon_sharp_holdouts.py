"""Unseen Sharp Ribbon loops, source seams, charts and mixed-turn chains."""
from .capture_family import capture
from .generate_ribbon import curve


def request():
    polygon = [[0.,0.,0.],[6.,0.,0.],[6.,6.,0.],[3.,6.,0.],[3.,3.,0.],[0.,3.,0.],[0.,0.,0.]]
    shifted = polygon[4:-1] + polygon[:5]
    shapes = [
        ('concave-loop',polygon,None), ('concave-seam',shifted,None),
        ('five-segments',[[0.,0.,0.],[5.,0.,0.],[8.,4.,0.],[13.,4.,0.],[10.,8.,0.],[15.,8.,0.]],None),
        ('double-turn',[[0.,0.,0.],[5.,0.,0.],[7.,4.,0.],[4.,7.,0.],[9.,7.,0.]],None),
        ('tilted-chain',[[2.,3.,4.],[7.,3.,4.],[7.,5.4,7.2],[12.,5.4,7.2]],None),
        ('uneven-chart',[[1.,2.,0.],[7.,2.,0.],[10.,6.,0.],[16.,6.,0.]],[-9.,-9.,-3.,2.,10.,10.]),
    ]
    operations=[]
    for name,points,knots in shapes:
        for both in [False,True]:
            operations.append(dict(op='ribbon_command',id=name+('-both'if both else''),
                curve=curve(points,knots=knots),distance=.25,side=[2.,-3.,0.],both_sides=both,
                corner='Sharp',loose=False,in_cplane=False,output_layer='Current'))
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__ == '__main__':
    capture('ribbon_sharp_holdouts',request(),['generate_ribbon_sharp_holdouts.py','generate_ribbon.py',
        'ribbon_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonSharpHoldouts20261109',timeout=1200)
