"""Cross-section rebuild definitions from independent SDK and actual commands."""
import copy
from .capture_family import capture


def request(command=False):
    polynomial=dict(type='nurbs',degree=2,knots=[0.,0.,0.,1.,1.,1.],
        control_points=[dict(point=p,weight=1.) for p in [[0.,0.,0.],[1.,2.,0.],[3.,0.,0.]]])
    circle=dict(type='circle',center=[0.,0.,0.],radius=2.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
    operations=[]
    def add(name,sections,count):
        operations.append(dict(op='sweep1',id=name,rail=dict(type='line',start=[0.,0.,0.],end=[0.,0.,5.]),
            sections=sections,parameters=[0.] if len(sections)==1 else [0.,5.],
            command=command,rebuild_point_count=count,refit_rail=True,inspect_definition=True))
    for name,source in [('open',polynomial),('closed',circle)]:
        for count in [0,1,2,3,5,10]:add(name+'-'+str(count),[source],count)
    rational=copy.deepcopy(polynomial)
    for control,weight in zip(rational['control_points'],[1.,.4,2.]):control['weight']=weight
    add('rational-5',[rational],5)
    add('polyline-5',[dict(type='polyline',vertices=[[0.,0.,0.],[1.,2.,0.],[3.,1.,0.]])],5)
    for name,source in [('open',polynomial),('closed',circle)]:
        second=copy.deepcopy(source)
        if name=='open':
            for control in second['control_points']:
                control['point']=[2.*control['point'][0],2.*control['point'][1],5.]
        else:second['center']=[0.,0.,5.];second['radius']=1.
        add('two-'+name+'-5',[source,second],5)
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),operations=operations)


if __name__=='__main__':
    capture('sweep1_rebuild_sdk',request(),[
        'generate_sweep1_rebuild.py','sweep1_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSweepRebuildSDK20261027',timeout=240)
