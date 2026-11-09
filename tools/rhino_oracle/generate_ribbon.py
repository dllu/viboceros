"""Native Ribbon geometry and command policy on independent curve families."""
import math
from .capture_family import capture


def curve(points,degree=1,weights=None,knots=None):
    n=len(points)
    return dict(degree=degree,control_points=[dict(point=p,weight=w) for p,w in zip(points,weights or [1.]*n)],
                knots=knots or [0.]*(degree+1)+[float(i) for i in range(1,n-degree)]+[float(n-degree)]*(degree+1))


def request():
    line=curve([[0.,0.,0.],[5.,0.,0.]])
    circle=curve([[3.,0.,0.],[3.,3.,0.],[0.,3.,0.],[-3.,3.,0.],[-3.,0.,0.],[-3.,-3.,0.],
                  [0.,-3.,0.],[3.,-3.,0.],[3.,0.,0.]],2,
                 [1.,math.sqrt(.5),1.,math.sqrt(.5),1.,math.sqrt(.5),1.,math.sqrt(.5),1.],
                 [0.,0.,0.,.25,.25,.5,.5,.75,.75,1.,1.,1.])
    corner=curve([[0.,0.,0.],[5.,0.,0.],[5.,4.,0.]])
    rectangle=curve([[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,4.,0.],[0.,0.,0.]])
    smooth=curve([[0.,0.,0.],[1.,2.,0.],[4.,-1.,0.],[5.,1.,0.]],3)
    operations=[]
    def add(name,c,side,**options):
        op=dict(op='ribbon_command',id=name,curve=c,distance=.5,side=side,both_sides=False,
                corner='Sharp',loose=False,in_cplane=False,output_layer='Current');op.update(options);operations.append(op)
    add('line',line,[2.,2.,0.]);add('line-both',line,[2.,2.,0.],both_sides=True)
    add('line-input-layer',line,[2.,-2.,0.],output_layer='Input')
    add('circle-outside',circle,[5.,0.,0.]);add('circle-inside',circle,[0.,0.,0.]);add('circle-both',circle,[5.,0.,0.],both_sides=True)
    for style in ['Sharp','Chamfer','Round','Smooth']:
        add('corner-'+style.lower(),corner,[7.,-2.,0.],corner=style)
    add('rectangle',rectangle,[7.,-2.,0.]);add('smooth',smooth,[0.,4.,0.]);add('smooth-loose',smooth,[0.,4.,0.],loose=True)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('ribbon',request(),['generate_ribbon.py','ribbon_probe.py','join_probe.py','merge_edges_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],'VibocerosOracleRibbon20261109',timeout=1200)
