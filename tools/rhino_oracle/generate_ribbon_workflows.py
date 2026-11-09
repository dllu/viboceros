"""Ribbon selection cleanup and construction-plane offset holdouts."""
from .capture_family import capture
from .generate_ribbon import curve


def request():
    line=curve([[0.,0.,3.],[5.,0.,6.]])
    tilted=curve([[0.,0.,0.],[1.,2.,2.],[4.,-1.,-1.],[5.,1.,1.]],3)
    spatial=curve([[0.,0.,0.],[1.,2.,2.],[4.,-1.,3.],[5.,1.,1.]],3)
    operations=[]
    for name,c,loose in [('line',line,False),('tilted',tilted,False),
        ('spatial',spatial,False),('tilted-loose',tilted,True),('spatial-loose',spatial,True)]:
        for in_plane in [False,True]:
            operations.append(dict(op='ribbon_command',id=name+('-cplane'if in_plane else '-curve-plane'),
                curve=c,distance=.5,side=[0.,5.,0.],both_sides=False,corner='Sharp',loose=loose,
                in_cplane=in_plane,output_layer='Input'))
    for layer in ['Current','Input']:
        operations.append(dict(op='ribbon_command',id='postselected-'+layer.lower(),curve=curve([[0.,0.,0.],[5.,0.,0.]]),
            distance=.5,side=[0.,5.,0.],both_sides=False,corner='Sharp',loose=False,in_cplane=False,
            output_layer=layer,postselected=True))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('ribbon_workflows',request(),['generate_ribbon_workflows.py','generate_ribbon.py','ribbon_probe.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonWorkflows20261109',timeout=1200)
