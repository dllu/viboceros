"""The same completion recipes with explicit pointer context and completed tails."""
from copy import deepcopy
if __package__:
    from .plane_finish_workflows import FINISH_WORKFLOWS, A, B, P, H
else:
    from plane_finish_workflows import FINISH_WORKFLOWS, A, B, P, H

POINTER_WORKFLOWS = {}
for name, case in FINISH_WORKFLOWS.items():
    case = deepcopy(case)
    if name.endswith('three_length_direction_enter'):
        case['actions'].extend([5,P]); case['expected_success'] = True
    if name.endswith('vertical_length_direction_enter'):
        case['actions'].extend([5,H]); case['expected_success'] = True
    if name in ['complete_three_choose_enter','complete_three_width_choose_enter','complete_midpoint_choose_enter','complete_center_three_choose_enter']:
        case['expected_success'] = True
    if name == 'complete_deform_empty': case['recovered'] = True
    case['pointer'] = True
    POINTER_WORKFLOWS[name.replace('complete_','pointer_',1)] = case

for view in ['Top','Front']:
    POINTER_WORKFLOWS['pointer_choose_'+view.lower()] = dict(actions=['3Point',A,B,'Enter','Enter'],
        expected_success=True,paced=True,pointer=True,view=view)
POINTER_WORKFLOWS['pointer_integer_getters'] = dict(actions=['Deformable','UDegree',2,'VDegree',4,'UPointCount',7,'VPointCount',6,A,[9,7,11]],expected_success=True,paced=True,pointer=True)
POINTER_WORKFLOWS['pointer_fractional_degree'] = dict(actions=['Deformable','UDegree=4.5',A,[9,7,11]],expected_success=True,paced=True,pointer=True)
