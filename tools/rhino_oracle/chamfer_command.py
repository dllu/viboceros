"""Prescribed public Chamfer getters, independent of measured output curves."""
A = dict(type='line', start=[0., 0., 0.], end=[4., 0., 0.])
B = dict(type='line', start=[4., 0., 0.], end=[4., 4., 0.])


def pick(index, point): return dict(curve=index, pick=point)
def case(actions, curves=None, **kwargs): return dict(curves=curves or [A, B], view='Top', actions=actions, **kwargs)
def configured(distances=(.5, 1.), join=False, trim=True):
    return ['Distances', distances[0], distances[1], 'Trim=Yes', 'Join=' + ('Yes' if join else 'No'), 'Trim=' + ('Yes' if trim else 'No')]

PICKS = [pick(0, [2.7, 0., 0.]), pick(1, [4., 1.7, 0.])]
PILOT = {
    'chamfer_initial_cancel': case(['Cancel'], pending_redo=True),
    'chamfer_parts': case(configured() + PICKS),
    'chamfer_joined': case(configured(join=True) + PICKS),
    'chamfer_untrimmed': case(configured(trim=False) + PICKS),
    'chamfer_cancel_second': case(configured() + [PICKS[0], 'Cancel'], pending_redo=True),
    'chamfer_distances_cancel': case(['Distances', .75, 'Cancel'], pending_redo=True),
}
CASES = dict(PILOT)
DISCOVERY = {
    'chamfer_preferences_setup': case(configured() + ['Cancel'], pending_redo=True),
    'chamfer_partial_distances_cancel': case(['Distances', .875, 'Cancel'], pending_redo=True),
    'chamfer_remember_partial_cancel': case(['Cancel'], pending_redo=True),
    'chamfer_second_distance_default': case(['Distances', 1.25, 'Enter'] + PICKS),
    'chamfer_numeric_cancel': case([.625, 'Cancel'], pending_redo=True),
    'chamfer_remember_numeric': case(['Cancel'], pending_redo=True),
    'chamfer_measured_distances': case(['Distances', [0., 0., 0.], [0., .5, 0.], [0., 0., 0.], [0., 1.25, 0.], 'Trim=Yes', 'Join=No'] + PICKS),
    'chamfer_after_first_pick': case(configured() + [PICKS[0], 'Distances', .25, .75, 'Join=Yes', PICKS[1]]),
    'chamfer_reverse_order': case(configured(join=True) + list(reversed(PICKS))),
    'chamfer_zero_parts': case(configured(distances=(0., 0.)) + PICKS),
    'chamfer_zero_joined': case(configured(distances=(0., 0.), join=True) + PICKS),
    'chamfer_zero_first': case(configured(distances=(0., 1.)) + PICKS),
    'chamfer_arc_extension': case(configured(distances=(.2, .3), join=True) + [pick(0, [0., 1., 0.]), pick(1, [-.5, 2.6, 0.])], curves=[dict(type='arc', points=[[1., 0., 0.], [2.**-.5, 2.**-.5, 0.], [0., 1., 0.]]), dict(type='line', start=[-.5, 2., 0.], end=[-.5, 3., 0.])]),
    'chamfer_nurbs_contact': case(configured(distances=(.2, .3)) + [pick(0, [1.5, .5625, 0.]), pick(1, [2., 1.9, 0.])], curves=[dict(type='nurbs', degree=2, control_points=[dict(point=p, weight=1.) for p in [[0., 0., 0.], [1., 0., 0.], [2., 1., 0.]]], knots=[0., 0., 0., 1., 1., 1.]), dict(type='line', start=[2., 1., 0.], end=[2., 5., 0.])]),
}
CASES.update(DISCOVERY)
HOLDOUTS = {
    'chamfer_shared_measurement_anchor': case(['Distances', [1., 2., 0.], [1., 2.5, 0.], [1., 3.25, 0.], 'Trim=Yes', 'Join=No'] + PICKS),
    'chamfer_measure_second_after_number': case(['Distances', .5, [1., 2., 0.], [1., 3., 0.], 'Trim=Yes', 'Join=No'] + PICKS),
    'chamfer_enter_defaults': case(configured() + ['Distances', 'Enter', 'Enter'] + PICKS),
    'chamfer_negative_distance_correction': case(['Distances', -1., .4, .8, 'Trim=Yes', 'Join=No'] + PICKS),
    'chamfer_units_arithmetic': case(['Distances', dict(number='0.02in'), dict(number='1/2'), 'Trim=Yes', 'Join=No'] + PICKS),
    'chamfer_measurement_cancel': case(['Distances', [1., 2., 0.], 'Cancel'], pending_redo=True),
    'chamfer_extension_menu_cancel': case(configured() + [PICKS[0], 'ExtendArcsBy', 'Cancel'], pending_redo=True),
    'chamfer_line_extension': case(configured(distances=(.2, .3), join=True) + ['ExtendArcsBy=Line', pick(0, [0., 1., 0.]), pick(1, [-.5, 2.6, 0.])], curves=DISCOVERY['chamfer_arc_extension']['curves']),
    'chamfer_spatial_joined': case(configured(join=True) + [pick(0, [3.2, 0., 1.6]), pick(1, [4., 1.7, 2.])], curves=[dict(type='line', start=[0., 0., 0.], end=[4., 0., 2.]), dict(type='line', start=[4., 0., 2.], end=[4., 4., 2.])]),
    'chamfer_oversized': case(configured(distances=(9., 1.)) + PICKS, pending_redo=True),
    'chamfer_zero_first_joined': case(configured(distances=(0., 1.), join=True) + PICKS),
}
CASES.update(HOLDOUTS)


def validate(op):
    if not isinstance(op, dict) or set(op) != {'op', 'id', 'case'} or op.get('op') != 'chamfer_command' or not isinstance(op.get('case'), str) or op['case'] not in CASES or op.get('id') != op['case']:
        raise ValueError('invalid prescribed Chamfer getter recipe')


def request(names=None):
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6), operations=[dict(op='chamfer_command', id=k, case=k) for k in (PILOT if names is None else names)])

if __package__:
    from . import chamfer_self_cases as _self
else:
    import chamfer_self_cases as _self
SELF_PILOT=_self.pilot(case,configured,pick)
SELF_DISCOVERY=_self.discovery(case,configured,pick)
SELF_HOLDOUTS=_self.holdouts(case,configured,pick)
CASES.update(SELF_PILOT);CASES.update(SELF_DISCOVERY);CASES.update(SELF_HOLDOUTS)
SELF_SPANS=_self.spans(case,configured,pick)
CASES.update(SELF_SPANS)

if __package__:
    from . import chamfer_closed_cases as _closed
else:
    import chamfer_closed_cases as _closed
CLOSED_PILOT=_closed.pilot(case,configured,pick)
CLOSED_HOLDOUTS=_closed.holdouts(case,configured,pick)
CASES.update(CLOSED_PILOT);CASES.update(CLOSED_HOLDOUTS)
CLOSED_FINAL=_closed.final(case,configured,pick)
CLOSED_BRANCHES=_closed.branches(case,configured,pick)
CASES.update(CLOSED_FINAL);CASES.update(CLOSED_BRANCHES)

if __package__:
    from . import chamfer_extension_cases as _extension
else:
    import chamfer_extension_cases as _extension
EXTENSION_PILOT=_extension.pilot(case,configured,pick)
EXTENSION_HOLDOUTS=_extension.holdouts(case,configured,pick)
CASES.update(EXTENSION_PILOT);CASES.update(EXTENSION_HOLDOUTS)
EXTENSION_STYLING=_extension.styling(case,configured,pick)
CASES.update(EXTENSION_STYLING)
EXTENSION_VERIFICATION=_extension.verification(case,configured,pick)
CASES.update(EXTENSION_VERIFICATION)
if __package__:
    from . import chamfer_branch_cases as _branch
else:
    import chamfer_branch_cases as _branch
BRANCH_PILOT=_branch.cases(EXTENSION_PILOT)
CASES.update(BRANCH_PILOT)
BRANCH_HOLDOUTS=_branch.holdouts(EXTENSION_PILOT)
CASES.update(BRANCH_HOLDOUTS)
BRANCH_SEAMS=_branch.seams(EXTENSION_PILOT)
CASES.update(BRANCH_SEAMS)
BRANCH_VERIFICATION=_branch.verification(EXTENSION_PILOT)
CASES.update(BRANCH_VERIFICATION)
