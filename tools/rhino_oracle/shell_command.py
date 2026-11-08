"""Authored public Shell getter recipes; output geometry is never an input."""
import copy

BOX = dict(type='box_brep', min=[0, 0, 0], max=[10, 8, 6])


def case(actions, openings=None, **options):
    return dict(curves=[copy.deepcopy(BOX)],
                openings=[openings or [[5, 4, 6]]], actions=actions,
                view='Top', shaded=True, pending_redo=True, **options)


PILOT = {
    'shell_getter_keep': case(['Thickness', 1, 'DeleteInput=No', 'Enter']),
    'shell_getter_delete': case(['Thickness', 1, 'DeleteInput=Yes', 'Enter']),
    'shell_getter_cancel': case(['Thickness', 1, 'Cancel']),
    'shell_getter_negative': case(['Thickness', -1, 1, 'DeleteInput=No', 'Enter']),
    'shell_getter_zero': case(['Thickness', 0, 1, 'DeleteInput=No', 'Enter']),
    'shell_getter_inline': case(['Thickness=0.5', 'DeleteInput=No', 'Enter']),
}
CASES = dict(PILOT)

QUALIFIED = {
    'shell_q_keep': case(['Thickness', 1, 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_delete': case(['Thickness', 1, 'DeleteInput=Yes', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_half': case(['Thickness', 0.5, 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_units': case(['Thickness', {'number': '0.05cm'}, 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_negative': case(['Thickness', -1, 1, 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_zero_keep': case(['Thickness', 0, 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_zero_delete': case(['Thickness', 0, 'DeleteInput=Yes', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_distance_cancel': case(['Thickness', 'Cancel', 'Cancel']),
    'shell_q_distance_enter': case(['Thickness', 'Enter', 1, 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_q_picked_cancel': case(['Thickness', 1, {'click': [5, 4, 6]}, 'Cancel']),
}
CASES.update(QUALIFIED)

HOLDOUTS = {
    'shell_h_thickness_enter': case(['Thickness', 2, 'Thickness', 'Enter', 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_h_selection_number': case([0.75, 'DeleteInput=No', {'click': [5, 4, 6]}, 'Enter']),
    'shell_h_zero_direct': case([0, 'DeleteInput=Yes', {'click': [5, 4, 6]}, 'Enter']),
    'shell_h_picked_thickness': case([{'click': [5, 4, 6]}, 'Thickness', 0.25, 'DeleteInput=No', 'Enter']),
    'shell_h_side': case(['Thickness', 1, 'DeleteInput=No', {'click': [10, 4, 3]}, 'Enter']),
}
HOLDOUTS['shell_h_side']['view'] = 'Right'
for zero in [False, True]:
    for delete in [False, True]:
        name = 'shell_h_batch_' + ('zero_' if zero else '') + ('delete' if delete else 'keep')
        recipe = case(['Thickness', 0 if zero else 0.5,
                       'DeleteInput=' + ('Yes' if delete else 'No'),
                       {'click': [5, 4, 6]}, {'click': [17, 4, 6]}, 'Enter'])
        recipe['curves'].append(dict(type='box_brep', min=[14, 0, 0], max=[20, 8, 6]))
        recipe['openings'].append([[17, 4, 6]])
        HOLDOUTS[name] = recipe
CASES.update(HOLDOUTS)


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
            or op['op'] != 'shell_command' or op['id'] != op['case']
            or op['case'] not in CASES):
        raise ValueError('invalid fixed Shell command recipe')


def request(names):
    operations = [dict(op='shell_command', id=name, case=name) for name in names]
    for op in operations:
        validate(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-8),
                operations=operations)
