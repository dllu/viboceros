"""Prompt-paced Plane completion discovery with an active-getter cancel tail.

The final Cancel is delivered only while a Plane prompt remains active. An
unused tail records early completion, without sending idle cleanup commands.
"""
FINISH_DISCOVERY = {
    'finish_initial_enter': dict(actions=['Enter', 'Cancel']),
    'finish_corner_enter': dict(actions=[[1,2,3], 'Enter', 'Cancel']),
    'finish_length_enter': dict(actions=[[1,2,3], 8, 'Enter', 'Cancel']),
    'finish_three_start_enter': dict(actions=['3Point', 'Enter', 'Cancel']),
    'finish_three_width_enter': dict(actions=['3Point', [1,2,3], [4,6,8], 'Enter', 'Cancel']),
    'finish_vertical_height_enter': dict(actions=['Vertical', [1,2,3], [5,8,9], 'Enter', 'Cancel']),
    'finish_center_three_enter': dict(actions=['Center', [1,2,3], '3Point', [4,6,8], 'Enter', 'Cancel']),
    'finish_midpoint_enter': dict(actions=['3Point', 'EdgeMidpoint', [1,2,3], [4,6,8], 'Enter', 'Cancel']),
}
for suffix in ['Yes', 'No']:
    FINISH_DISCOVERY['finish_deform_' + suffix.lower()] = dict(actions=[
        'Deformable=' + suffix, 'UDegree=3', 'VDegree=2', 'UPointCount=6',
        'VPointCount=5', [1,2,3], [9,7,11], 'Cancel'])
for case in FINISH_DISCOVERY.values():
    case['paced'] = True
