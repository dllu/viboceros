"""Public Offset Loose=Yes output, keeping every source and raw curve basis."""
import copy
from .capture_family import capture
from .generate_ribbon import request as ribbon_request,curve


def request():
    root=copy.deepcopy(ribbon_request())
    for op in root['operations']:
        op['op']='loose_offset_command';op['id']='loose-'+op['id'];op['loose']=True
    template=root['operations'][0]
    shapes=[('rational',curve([[0.,0.,0.],[1.,2.,0.],[4.,-1.,0.],[5.,1.,0.]],3,[1.,.7,1.3,1.])),
            ('quadratic',curve([[0.,0.,0.],[1.,2.,0.],[3.,3.,0.],[5.,1.,0.]],2)),
            ('tilted',curve([[0.,0.,0.],[1.,2.,2.],[4.,-1.,-1.],[5.,1.,1.]],3)),
            ('nonuniform',curve([[0.,0.,0.],[1.,2.,0.],[2.,3.,0.],[3.,0.,0.],[4.,-1.,0.],[5.,1.,0.]],3,
                                  knots=[0.]*4+[.15,.8]+[1.]*4))]
    for name,shape in shapes:
        op=copy.deepcopy(template);op.update(id='loose-'+name,curve=shape,side=[0.,4.,0.]);root['operations'].append(op)
    return root


if __name__=='__main__':
    capture('curve_loose_offset',request(),['generate_curve_loose_offset.py','generate_ribbon.py','ribbon_probe.py',
        'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleCurveLooseOffset20261109',timeout=1200)
