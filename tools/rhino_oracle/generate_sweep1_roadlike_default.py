"""Roadlike input families and explicit-axis command reference definitions."""
import math
from .capture_family import capture


def request(command=True, explicit_axes=False):
    root2 = math.sqrt(2.)
    examples = [
        ('xy', [[3.,0.,0.],[3./root2,3./root2,0.],[0.,3.,0.]], [0.,0.,1.]),
        ('xz', [[3.,0.,0.],[3./root2,0.,3./root2],[0.,0.,3.]], [0.,1.,0.]),
        ('yz', [[0.,3.,0.],[0.,3./root2,3./root2],[0.,0.,3.]], [1.,0.,0.]),
    ]
    u = [1./root2,1./root2,0.]
    v = [-1./math.sqrt(6.),1./math.sqrt(6.),2./math.sqrt(6.)]
    origin = [1.,2.,3.]
    points = [[origin[i]+3.*(a*u[i]+b*v[i]) for i in range(3)]
              for a,b in [(1.,0.),(1./root2,1./root2),(0.,1.)]]
    examples.append(('oblique', points, [1./math.sqrt(3.),-1./math.sqrt(3.),1./math.sqrt(3.)]))
    operations = []
    for name, points, direction in examples:
        rail = dict(type='arc',points=points)
        for refit in [False, True]:
            operations.append(dict(op='sweep1',id=name+('-refit' if refit else '-retained'),
                rail=rail,sections=[dict(type='line',start=points[0],
                    end=[points[0][i]+direction[i] for i in range(3)])],
                parameters=[0.],roadlike=True,command=command,refit_rail=refit,inspect_definition=True))
    for name, end, profile in [('line-x',[5.,0.,0.],[0.,1.,0.]),('line-z',[0.,0.,5.],[1.,0.,0.])]:
        operations.append(dict(op='sweep1',id=name,rail=dict(type='line',start=[0.,0.,0.],end=end),
            sections=[dict(type='line',start=[0.,0.,0.],end=profile)],parameters=[0.],
            roadlike=True,command=command,refit_rail=False,inspect_definition=True))
    operations.append(dict(op='sweep1',id='spatial',rail=dict(type='nurbs',degree=3,
        knots=[0.,0.,0.,0.,1.,1.,1.,1.],control_points=[dict(point=p,weight=1.) for p in
            [[0.,0.,0.],[2.,1.,1.],[-1.,3.,4.],[0.,5.,5.]]]),
        sections=[dict(type='line',start=[0.,0.,0.],end=[1.,0.,0.])],parameters=[0.],
        roadlike=True,command=command,refit_rail=False,inspect_definition=True))
    if explicit_axes:
        normals={name: direction for name, points, direction in examples}
        for operation in operations:
            name=operation['id'].split('-')[0]
            operation['roadlike_axis']=normals.get(name, [0.,0.,1.])
            if operation['id']=='line-z':operation['roadlike_axis']=[0.,1.,0.]
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),operations=operations)


if __name__ == '__main__':
    capture('sweep1_roadlike_explicit',request(explicit_axes=True),[
        'generate_sweep1_roadlike_default.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleRoadlikeDefault20261027',timeout=300)
