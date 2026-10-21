"""Owned subdivision-cone reference contacts, point fitting and source history."""
from copy import deepcopy
if __package__:
 from .subd_cylinder_references_probe import ALL_CASES as CYLINDER_CASES, PREFIX, LINES
else:
 from subd_cylinder_references_probe import ALL_CASES as CYLINDER_CASES, PREFIX, LINES
SCENES={k.replace('cylinder_','cone_'):deepcopy(v)for k,v in CYLINDER_CASES.items()}
for case in SCENES.values():
 if case['actions'][-1]=='Cancel':case['cancel']=True
SCENES['cone_tangent_circle_line']=dict(live=True,actions=PREFIX+['Tangent',dict(curve=0,pick=[1.2,1.6,0.]),dict(curve=1,pick=[1.,8.,0.]),'Enter',5.],curves=[dict(type='circle',center=[0.,0.,0.],radius=2.,x_axis=[1.,0.,0.],normal=[0.,0.,1.]),dict(type='line',start=[-10.,8.,0.],end=[14.,8.,0.])])
SCENES['cone_tangent_base_angle']=deepcopy(SCENES['cone_tangent_radius'])
SCENES['cone_tangent_base_angle']['actions'][-1]='OrBaseAngle=30'

def request():return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_cone_references',id=k,case=k)for k in SCENES])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_cone_references'and op['case']in SCENES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDCone reference recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_cylinder_references_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDCone',SCENES[op['case']])
