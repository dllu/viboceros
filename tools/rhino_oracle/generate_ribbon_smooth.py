"""Ordinary smooth Ribbon curves, analytic circles/arcs, gauges and charts."""
import copy
import math
from .capture_family import capture
from .generate_ribbon import curve, request as originals


def request():
    base=originals(); operations=[]
    shapes={
        'cubic':curve([[0.,0.,0.],[1.,2.,0.],[4.,-1.,0.],[5.,1.,0.]],3),
        'rational':curve([[0.,0.,0.],[1.,2.,0.],[4.,-1.,0.],[5.,1.,0.]],3,[1.,.7,1.3,1.]),
        'quadratic':curve([[0.,0.,0.],[1.,2.,0.],[3.,3.,0.],[5.,1.,0.]],2),
        'nonuniform':curve([[0.,0.,0.],[1.,2.,0.],[2.,3.,0.],[3.,0.,0.],[4.,-1.,0.],[5.,1.,0.]],3,
            knots=[0.]*4+[.15,.8]+[1.]*4),
        'circle':copy.deepcopy(next(o['curve']for o in base['operations']if o['id']=='circle-outside')),
        'arc':curve([[3.,0.,0.],[3.,3.,0.],[0.,3.,0.]],2,[1.,math.sqrt(.5),1.]),
    }
    def add(name,shape,distance=.15,side=None,both=False,kind='nurbs'):
        operations.append(dict(op='ribbon_command',id=name,curve=copy.deepcopy(shape),distance=distance,
            side=side or [0.,4.,0.],both_sides=both,corner='Sharp',loose=False,in_cplane=False,
            output_layer='Current',native_kind=kind))
    for name,shape in shapes.items():
        for label,side,both in [('left',[0.,4.,0.],False),('right',[2.,-4.,0.],False),('both',[0.,4.,0.],True)]:
            add(name+'-'+label,shape,.1 if name=='nonuniform'else.15,side,both)
    for name in ['cubic','rational','circle','arc']:
        for gauge in [.5,2.,10.]:
            shape=copy.deepcopy(shapes[name])
            for p in shape['control_points']:p['weight']*=gauge
            add(name+'-gauge-'+str(gauge),shape,both=True)
        shape=copy.deepcopy(shapes[name]);shape['domain']=[-4.,8.]
        add(name+'-chart',shape,both=True)
    shape=copy.deepcopy(shapes['cubic'])
    for p in shape['control_points']:
        x,y,z=p['point'];p['point']=[2.+x,3.+.6*y,4.+.8*y]
    add('tilted-cubic',shape,side=[2.,5.4,7.2],both=True)
    shape=copy.deepcopy(shapes['cubic']);shape['control_points'].reverse()
    add('reversed-cubic',shape,both=True)
    add('native-circle',shapes['circle'],both=True,kind='circle')
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('ribbon_smooth',request(),['generate_ribbon_smooth.py','generate_ribbon.py','ribbon_probe.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonSmooth20261109',timeout=1200)
