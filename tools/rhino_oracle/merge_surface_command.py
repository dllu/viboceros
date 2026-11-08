"""Authored MergeSrf command workflows, separate from public SDK observations."""
import copy
if __package__:
    from .merge_surface_cases import pair, HOLDOUTS as SDK_HOLDOUTS
else:
    from merge_surface_cases import pair, HOLDOUTS as SDK_HOLDOUTS


def case(actions, **changes):
    value = dict(curves=pair()['sources'], actions=actions, view='Top', preselected=True,
                 selection_order=[0, 1], pending_redo=True)
    value.update(changes)
    return copy.deepcopy(value)


PILOT = {
    'merge_cmd_pre_sharp': case(['Smooth=No', 'Roundness=1', 'Tolerance=0.0001', 'Enter']),
    'merge_cmd_pre_smooth': case(['Smooth=Yes', 'Roundness=0.5', 'Tolerance=0.0001', 'Enter']),
    'merge_cmd_cancel': case(['Smooth=No', 'Roundness=0.25', 'Tolerance=0.0002', 'Cancel']),
}
CASES = copy.deepcopy(PILOT)

DISCOVERY = {
    'merge_d_pre_default': case([], common_group=False),
    'merge_d_post_sharp': case(['Smooth=No', 'Roundness=1', 'Tolerance=0.0001',
                              {'curve': 0}, {'curve': 1}, 'Enter'],
                             preselected=False, common_group=False),
    'merge_d_post_smooth': case(['Smooth=Yes', 'Roundness=0.5', 'Tolerance=0.0001',
                               {'curve': 0}, {'curve': 1}, 'Enter'],
                              preselected=False, common_group=False),
    'merge_d_cancel': case(['Smooth=No', 'Roundness=0.25', 'Tolerance=0.0002', 'Cancel'],
                          preselected=False, common_group=False),
    'merge_d_pre_after_cancel': case([], common_group=False),
    'merge_d_cancel_one_pick': case([{'curve': 0}, 'Cancel'],
                                   preselected=False, common_group=False),
}
CASES.update(copy.deepcopy(DISCOVERY))

QUALIFIED = {}
for name, recipe in DISCOVERY.items():
    value = copy.deepcopy(recipe)
    if name in ['merge_d_post_sharp', 'merge_d_post_smooth']: value['actions'].pop()
    QUALIFIED[name.replace('merge_d_', 'merge_q_')] = value
QUALIFIED['merge_q_post_reverse'] = case(['Smooth=Yes', 'Roundness=0.5', 'Tolerance=0.0001',
    {'curve': 1}, {'curve': 0}], preselected=False, common_group=False)
QUALIFIED['merge_q_pre_reverse'] = case([], selection_order=[1, 0], common_group=False)
QUALIFIED['merge_q_partial_cancel'] = case(['Cancel'], selection_order=[0], common_group=False)
QUALIFIED['merge_q_getters'] = case(['Smooth=Yes', 'Roundness', {'number': '1/4'},
    'Tolerance', {'number': '0.0001'}, {'curve': 0}, {'curve': 1}],
    preselected=False, common_group=False)
CASES.update(copy.deepcopy(QUALIFIED))

GETTERS = {
    'merge_g_fraction': copy.deepcopy(QUALIFIED['merge_q_getters']),
    'merge_g_tolerance_cancel': case(['Tolerance', 'Cancel'], preselected=False, common_group=False),
    'merge_g_tolerance_range': case(['Smooth=No', 'Roundness=1', 'Tolerance',
        {'number': '0'}, {'number': '0.0000005'}, {'number': '0.0001'},
        {'curve': 0}, {'curve': 1}], preselected=False, common_group=False),
    'merge_g_roundness_range': case(['Smooth=Yes', 'Roundness', -0.25, 1.25, 0.5,
        {'curve': 0}, {'curve': 1}], preselected=False, common_group=False),
    'merge_g_changed_document': case(['Smooth=No', 'Roundness=1', {'curve': 0}, {'curve': 1}],
        preselected=False, common_group=False, absolute_tolerance=0.001),
}
CASES.update(copy.deepcopy(GETTERS))

HOLDOUTS = {
    'merge_h_document_high': case(['Smooth=Yes', {'curve': 0}, {'curve': 1}],
        preselected=False, common_group=False, absolute_tolerance=0.001),
    'merge_h_document_low_cancel': case(['Cancel'], preselected=False, common_group=False),
    'merge_h_gap_failure': case(['Smooth=No','Roundness=1','Tolerance=0.0001',{'curve':0},{'curve':1}],
        curves=pair(gap=.01)['sources'], preselected=False, common_group=False),
    'merge_h_bowed_failure': case(['Smooth=No','Roundness=1','Tolerance=0.0001',{'curve':0},{'curve':1}],
        curves=SDK_HOLDOUTS['merge_bowed_boundary']['sources'], preselected=False, common_group=False),
    'merge_h_shared_pre': case([]),
    'merge_h_sel_last': case(['Smooth=No','Roundness=1','Tolerance=0.0001',{'curve':0},{'curve':1}],
        preselected=False, common_group=False, capture_recall=True),
}
CASES.update(copy.deepcopy(HOLDOUTS))


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
            or op.get('op') != 'merge_surface_command' or op.get('id') != op.get('case')
            or op.get('case') not in CASES):
        raise ValueError('invalid prescribed MergeSrf command case')


def request(names):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6),
                operations=[dict(op='merge_surface_command', id=name, case=name) for name in names])
