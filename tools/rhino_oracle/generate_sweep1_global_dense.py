"""Planar curved rails expose dense scalar Global interpolation samples."""
import math
from .capture_family import capture


def request():
    positions = [[0.,3.,0.],[0.,3./math.sqrt(2),3./math.sqrt(2)],[0.,0.,3.]]
    operations = []
    for name, widths in [('base',[2.,2.,2.]),('first',[3.,2.,2.]),
                          ('middle',[2.,3.,2.]),('last',[2.,2.,3.]),
                          ('combination',[1.25,3.5,2.75])]:
        operations.append(dict(op='sweep1',id=name,
            rail=dict(type='arc',points=positions),
            sections=[dict(type='line',start=p,end=[w,p[1],p[2]])
                      for w,p in zip(widths,positions)],
            parameters=[0.,3.*math.pi/4,3.*math.pi/2],blend=1,inspect_definition=True))
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),operations=operations)


if __name__=='__main__':
    capture('sweep1_global_dense',request(),[
        'generate_sweep1_global_dense.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepGlobalDense20261027',timeout=240)
