"""Owned subdivision truncated-cone reference contacts, point fitting and source history."""
from copy import deepcopy
if __package__:
 from .subd_cylinder_references_probe import ALL_CASES as CYLINDER_CASES, PREFIX, LINES
else:
 from subd_cylinder_references_probe import ALL_CASES as CYLINDER_CASES, PREFIX, LINES
SCENES={k.replace('cylinder_','truncated_'):deepcopy(v)for k,v in CYLINDER_CASES.items()}
for case in SCENES.values():
 if case['actions'][-1]=='Cancel':case['cancel']=True
SCENES['truncated_tangent_circle_line']=dict(live=True,actions=PREFIX+['Tangent',dict(curve=0,pick=[1.2,1.6,0.]),dict(curve=1,pick=[1.,8.,0.]),'Enter',5.],curves=[dict(type='circle',center=[0.,0.,0.],radius=2.,x_axis=[1.,0.,0.],normal=[0.,0.,1.]),dict(type='line',start=[-10.,8.,0.],end=[14.,8.,0.])])

for case in SCENES.values():
 case['actions'].insert(0,'DirectionConstraint=Vertical')
 if case['actions'][-1]!='Cancel':case['actions'].append(1.)
 if case['actions'][-2:] == [5.,1.] and case.get('no_output'):
  case.pop('no_output');case['degenerate_fit']=True
SCENES['truncated_tangent_cancel_end']=deepcopy(SCENES['truncated_tangent_points'])
SCENES['truncated_tangent_cancel_end']['actions'][-1]='Cancel'
SCENES['truncated_tangent_cancel_end']['cancel']=True
SCENES['truncated_fit_cancel_end']=deepcopy(SCENES['truncated_fit_postselected'])
SCENES['truncated_fit_cancel_end']['actions'][-1]='Cancel'
SCENES['truncated_fit_cancel_end']['cancel']=True

def request():return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_truncated_cone_references',id=k,case=k)for k in SCENES])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_truncated_cone_references'and op['case']in SCENES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDTruncatedCone reference recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_cylinder_references_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDTruncatedCone',SCENES[op['case']])
