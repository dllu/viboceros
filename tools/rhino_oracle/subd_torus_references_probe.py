"""Owned torus tangent, fitted and curve-guided bases with source history."""
from copy import deepcopy
if __package__:
 from .subd_cylinder_references_probe import ALL_CASES as BASE_CASES
 from .subd_sphere_references_probe import SCENES as SPHERE_CASES
else:
 from subd_cylinder_references_probe import ALL_CASES as BASE_CASES
 from subd_sphere_references_probe import SCENES as SPHERE_CASES
PREFIX=['AroundFaces=8','VerticalFaces=6']
SCENES={}
for key,value in BASE_CASES.items():
 case=deepcopy(value);case['actions']=PREFIX+case['actions'][4:]
 if case['actions'][-1]=='Cancel':case['cancel']=True
 else:case['actions'][-1:]=['FixInnerDimension=No',1.]
 if key.endswith('_finish'):case['diagnostic']=True
 SCENES[key.replace('cylinder_','torus_').replace('cancel_height','cancel_tube')]=case
for key,value in SPHERE_CASES.items():
 if not key.startswith('sphere_around_'):continue
 case=deepcopy(value);case['actions']=PREFIX+case['actions'][2:]+['FixInnerDimension=No',1.]
 SCENES[key.replace('sphere_','torus_')]=case
SCENES['torus_around_circle']['actions'][0]='AroundFaces=7'
SCENES['torus_fit_collinear_fixed']=deepcopy(SCENES['torus_fit_collinear_finish'])
SCENES['torus_fit_collinear_fixed']['actions'][-2]='FixInnerDimension=Yes'
SCENES['torus_fit_coincident_fixed']=deepcopy(SCENES['torus_fit_coincident_finish'])
SCENES['torus_fit_coincident_fixed']['actions'][-2]='FixInnerDimension=Yes'
SCENES['torus_tangent_circle_line']=dict(live=True,actions=PREFIX+['Tangent',dict(curve=0,pick=[1.2,1.6,0.]),dict(curve=1,pick=[1.,8.,0.]),'Enter','FixInnerDimension=Yes',1.],curves=[dict(type='circle',center=[0.,0.,0.],radius=2.,x_axis=[1.,0.,0.],normal=[0.,0.,1.]),dict(type='line',start=[-10.,8.,0.],end=[14.,8.,0.])])

# Invalid numeric input leaves the native getter active. End these recovery
# recipes explicitly so the capture never relies on external cancellation.
for key,case in SCENES.items():
 case.pop('diagnostic',None)
 if key.endswith('_finish'):
  case['actions'].append('Cancel');case['cancel']=True

def request():return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_torus_references',id=k,case=k)for k in SCENES])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_torus_references'and op['case']in SCENES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDTorus reference recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_cylinder_references_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDTorus',SCENES[op['case']])
