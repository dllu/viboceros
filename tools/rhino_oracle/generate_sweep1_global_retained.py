"""Retained dense straight rails separate scalar blending from rail fitting."""
from .capture_family import capture


def request():
    degree = 3
    knots = [0.]*4+[5.*i/32 for i in range(1,32)]+[5.]*4
    count = len(knots)-degree-1
    rail = dict(type='nurbs', degree=degree, knots=knots,
        control_points=[dict(point=[0.,0.,sum(knots[i+1:i+4])/3],weight=1.)
                        for i in range(count)])
    operations = []
    for name, widths, blend in [('base',[2.,2.,2.],1),('first',[3.,2.,2.],1),
          ('middle',[2.,3.,2.],1),('last',[2.,2.,3.],1),
          ('combination',[1.25,3.5,2.75],1),('local',[2.,1.,3.],0)]:
        operations.append(dict(op='sweep1',id=name,rail=rail,
            sections=[dict(type='line',start=[0.,0.,t],end=[w,0.,t])
                      for t,w in zip([0.,2.5,5.],widths)],
            parameters=[0.,2.5,5.],blend=blend,command=True,refit_rail=False,
            inspect_definition=True))
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),operations=operations)


if __name__=='__main__':
    capture('sweep1_global_retained',request(),[
        'generate_sweep1_global_retained.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepGlobalRetained20261027',timeout=240)
