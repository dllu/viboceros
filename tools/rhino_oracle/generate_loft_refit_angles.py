"""Explicit corner angles for straight, shallow, obtuse and closed profiles."""
import copy
import json
import math
from .capture_family import HERE, capture


def request():
    original = json.loads((HERE/'fixtures/loft_refit_polylines.json').read_text())
    sources = {}
    for name in ['corner','small_kink','rectangle','backtrack','rational','unclamped']:
        key='loft-refit-polyline-'+name+'-straight-0.001'
        sources[name]=copy.deepcopy(next(o['curves'] for o in original['operations'] if o['id']==key))
    for name, degrees in [('acute',30.),('obtuse',150.)]:
        angle=math.radians(degrees)
        source=dict(degree=1,knots=[0.,0.,1.,2.,2.],control_points=[
            dict(point=p,weight=1.) for p in [[0.,0.,0.],[1.,0.,0.],
                [1.+math.cos(angle),math.sin(angle),0.]]])
        curves=[]
        for index in range(3):
            curve=copy.deepcopy(source)
            for cp in curve['control_points']:cp['point'][2]+=2.*index
            curves.append(curve)
        sources[name]=curves
    line=dict(degree=3,knots=[0.]*4+[1.]*4,
        control_points=[dict(point=[x,0.,0.],weight=1.) for x in [0.,.2,.7,3.]])
    sources['line']=[]
    for index in range(3):
        curve=copy.deepcopy(line)
        for cp in curve['control_points']:cp['point'][2]+=2.*index
        sources['line'].append(curve)
    angles=[None,0.,1e-6,math.pi/180.,math.pi/6.,math.pi/2.,5.*math.pi/6.,math.pi]
    operations=[]
    for name,curves in sources.items():
        for limit in [0.,.001]:
            for index,angle in enumerate(angles):
                op=dict(op='loft',id='loft-refit-angle-{}-{}-{}'.format(name,limit,index),
                    curves=copy.deepcopy(curves),style='straight',closed=False,
                    refit_tolerance=limit,inspect_inputs=True,sample_geometry=True,allow_empty=True)
                if angle is not None:op['refit_angle_tolerance']=angle
                operations.append(op)
    for name in ['corner','small_kink']:
        for style in ['normal','loose','tight','uniform']:
            for index in [0,1,2,3]:
                op=copy.deepcopy(next(o for o in operations if o['id']=='loft-refit-angle-{}-0.001-{}'.format(name,index)))
                op['id']+='-'+style;op['style']=style;operations.append(op)
    original['operations']=operations
    return original


if __name__=='__main__':
    capture('loft_refit_angles',request(),['generate_loft_refit_angles.py',
        'fixtures/loft_refit_polylines.json','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleLoftRefitAngles20261026')
