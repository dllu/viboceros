"""Capture Stretch's public point map and command input modes."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request(pilot=False):
    points=[[x,y,z]for x,y in [(0.,0.),(2.,-3.)]for z in [-10.,-1.,0.,.01,1.,2.5,5.,7.5,9.,9.99,10.,11.,20.]]
    ops=[]
    def sdk(name,**kw):ops.append(dict(op='stretch_sdk',id=name,start=kw.pop('start',[0.,0.,0.]),end=kw.pop('end',[0.,0.,10.]),points=points,**kw))
    for length in [-20.,-1.,0.,.01,2.5,10.,20.,100.]:sdk('length_'+str(length).replace('-','minus').replace('.','_'),length=length)
    for target in [[0.,0.,20.],[2.,3.,20.],[2.,3.,5.],[2.,3.,0.],[2.,3.,-5.],[0.,0.,0.]]:sdk('target_'+str(len(ops)),target=target)
    for start,end in [([0.,0.,0.],[10.,0.,0.]),([0.,0.,10.],[0.,0.,0.]),([1.,2.,3.],[5.,6.,11.]),([0.,0.,0.],[0.,0.,0.]),([0.,0.,0.],[0.,0.,1e-12])]:sdk('axis_'+str(len(ops)),start=start,end=end,length=20.)
    for mode in ['factor','length','target']:
        for copy,rigid,grouped in [(False,False,False),(True,False,False),(False,True,False),(False,True,True)]:
            ops.append(dict(op='stretch_command',id=mode+'_'+str(len(ops)),start=[0.,0.,0.],end=[0.,0.,10.],points=[[2.,1.,z]for z in [-2.,0.,2.5,5.,7.5,10.,12.]],copy=copy,rigid=rigid,grouped=grouped,**{mode:2. if mode=='factor'else 20. if mode=='length'else [3.,4.,20.]}))
    for length in [2.**-32*(1.-1e-6),2.**-32,2.**-32*(1.+1e-6),1e-8]:
        sdk('threshold_'+str(len(ops)),end=[0.,0.,length],length=20.)
    for shape in ('Points','Line','Curve','Surface','Box','Mesh'):
        for copy,rigid,grouped in [(False,False,False),(True,False,True),(False,True,False),(True,True,True)]:
            ops.append(dict(op='stretch_geometry_command',id='geometry_'+shape+'_'+str(len(ops)),shape=shape,copy=copy,rigid=rigid,grouped=grouped,factor=2.))
    for axis in ('Reverse','Spatial','Skew'):
        for shape in ('Points','Curve','Surface','Box'):
            ops.append(dict(op='stretch_geometry_command',id='axis_'+axis+'_'+shape,shape=shape,axis=axis,copy=False,rigid=False,grouped=False,factor=.5))
        ops.append(dict(op='stretch_geometry_command',id='rigid_'+axis,shape='Curve',axis=axis,copy=False,rigid=True,grouped=True,factor=2.))
    for factor in (0.,-1.,1.,.25):
        ops.append(dict(op='stretch_geometry_command',id='factor_boundary_'+str(factor).replace('-','minus').replace('.','_'),shape='Points',copy=False,rigid=False,grouped=False,factor=factor))
    for key,value in [('length',0.),('length',-1.),('target',[2.,3.,-5.])]:
        ops.append(dict(op='stretch_geometry_command',id='target_boundary_'+str(len(ops)),shape='Points',copy=False,rigid=False,grouped=False,**{key:value}))
    for shape in ('Points','Curve','Surface','Box'):
        for key,value in [('length',15.),('target',[3.,4.,15.])]:
            ops.append(dict(op='stretch_geometry_command',id=key+'_'+shape,shape=shape,copy=False,rigid=False,grouped=False,**{key:value}))
    ops.append(dict(op='stretch_geometry_command',id='early_factor',shape='Points',copy=False,rigid=False,grouped=False,factor=2.,early_factor=True))
    for finish in ('Complete','Cancel'):
        ops.append(dict(op='stretch_geometry_command',id='repeat_'+finish,shape='Curve',copy=True,rigid=False,grouped=True,factor=2.,targets=[.5,1.5],finish=finish))
    for copy in (False,True):
        ops.append(dict(op='stretch_command',id='identity_'+str(int(copy)),start=[0.,0.,0.],end=[0.,0.,10.],points=[[0.,0.,5.]],copy=copy,rigid=False,grouped=False,factor=1.))
    for shape in ('curve','rational','closed','periodic','line','circle','arc','surface','surface_closed','mesh'):
        for copy in (False,True):
            for rigid in (False,True):
                ops.append(dict(op='stretch_grips',id='grips_'+shape+'_'+str(int(copy))+str(int(rigid)),source=shape,selected=[0,1],copy=copy,rigid=rigid))
    for parent,point in [(True,False),(False,True)]:
        for copy in (False,True):
            ops.append(dict(op='stretch_grips',id='mixed_'+str(int(parent))+str(int(copy)),source='rational',selected=[0,2],parent=parent,point=point,copy=copy,rigid=False))
    for shape in ('rational','surface'):
        ops.append(dict(op='stretch_grips',id='grips_repeat_'+shape,source=shape,selected=[0,1],copy=True,rigid=False,targets=[.5,1.5]))
    for copy in (False,True):
        ops.append(dict(op='stretch_command',id='postselected_'+str(int(copy)),start=[0.,0.,0.],end=[0.,0.,10.],points=[[2.,1.,z]for z in [-2.,2.5,10.,12.]],copy=copy,rigid=False,grouped=False,factor=2.,preselect=False))
    if pilot:
        ops=[o for o in ops if o['id'].startswith(('identity_','grips_rational','grips_surface_','mixed_'))]
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    import sys
    pilot='--pilot'in sys.argv;q=request(pilot);p=Path('/tmp/viboceros-stretch-pilot-request.json')if pilot else HERE/'fixtures/stretch.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleStretch20261020').run_rhino(q,timeout=300. if pilot else 900.)
    assert len(r['results'])==len(q['operations'])and all('error'not in row for row in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();r['provenance']=dict(request_sha256=digest(p),sources={n:digest(HERE/n)for n in ('generate_stretch.py','stretch_probe.py','stretch_geometry_probe.py','stretch_grips_probe.py','grip_transform_probe.py','taper_command_probe.py','twist_command_probe.py','number_token.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')})
    out=Path('/tmp/viboceros-stretch-pilot-observations.json')if pilot else HERE/'observations/stretch.json';out.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded Stretch cases',len(r['results']),flush=True)
if __name__=='__main__':main()
