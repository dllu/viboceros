"""Closed-profile Point sweeps retain complete rational surface definitions."""
from .capture_family import capture


def request():
    operations=[]
    for name, z, start in [('closed-end',0.,None),('closed-both',2.5,[0.,0.,0.])]:
        op=dict(op='sweep1',id=name,
            rail=dict(type='line',start=[0.,0.,0.],end=[0.,0.,5.]),
            sections=[dict(type='circle',center=[0.,0.,z],radius=2.,
                           x_axis=[1.,0.,0.],normal=[0.,0.,1.])],
            parameters=[z],blend=0,command=True,refit_rail=True,
            end_point=[0.,0.,5.],inspect_definition=True)
        if start is not None:op['start_point']=start
        operations.append(op)
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),operations=operations)


if __name__=='__main__':
    capture('sweep1_point_ends_closed',request(),[
        'generate_sweep1_point_ends_closed.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepPointClosed20261027',timeout=240)
