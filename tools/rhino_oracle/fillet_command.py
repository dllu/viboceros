"""Prescribed curve sources and public Fillet getter inputs."""
A = dict(type='line', start=[0., 0., 0.], end=[4., 0., 0.])
B = dict(type='line', start=[4., 0., 0.], end=[4., 4., 0.])


def pick(index, point):
    return dict(curve=index, pick=point)


def case(actions, curves=None, **kwargs):
    return dict(curves=curves or [A, B], view='Top', actions=actions, **kwargs)


PICKS = [pick(0, [3.7, 0., 0.]), pick(1, [4., .3, 0.])]
PILOT = {
    'fixed_joined': case(['Radius=0.5', 'Join=Yes', 'Trim=Yes', 'Dynamic=No'] + PICKS),
    'dynamic_cancel': case(['Radius=0.5', 'Join=No', 'Trim=Yes', 'Dynamic=Yes'] + PICKS + ['Cancel'], pending_redo=True),
    'cancel_first': case(['Dynamic=No', 'Cancel'], pending_redo=True),
}
CASES = dict(PILOT)
DISCOVERY = {
    'ordered_joined': case(['Radius', .5, 'Join=Yes', 'Trim=Yes', 'Dynamic=No'] + PICKS),
    'ordered_parts': case(['Radius', .75, 'Join=No', 'Trim=Yes', 'Dynamic=No'] + PICKS),
    'dynamic_ready_cancel': case(['Radius', .5, 'Join=No', 'Trim=Yes', 'Dynamic=Yes'] + PICKS + ['Cancel'], pending_redo=True),
}
CASES.update(DISCOVERY)


def configured(dynamic=False, radius=.5, join=False, trim=True):
    return ['Radius', radius, 'Join=' + ('Yes' if join else 'No'),
            'Trim=' + ('Yes' if trim else 'No'), 'Dynamic=' + ('Yes' if dynamic else 'No')]


def shot(label):
    return dict(view_snapshot=label)


HOLDOUTS = {
    'dynamic_enter': case(configured(True) + PICKS + [shot('ready'), 'Enter']),
    'dynamic_numeric': case(configured(True, join=True) + PICKS + [1.25]),
    'dynamic_distance': case(configured(True) + PICKS + [[0., 0., 0.], dict(hover=[0., 1.5, 0.]), shot('distance'), [0., 1.5, 0.]]),
    'dynamic_options': case(configured(True) + PICKS + ['Join=Yes', 'Trim=No', shot('untrimmed'), 'Enter']),
    'dynamic_distance_cancel': case(configured(True) + PICKS + [[0., 0., 0.], dict(hover=[0., 1., 0.]), shot('cancel_distance'), 'Cancel'], pending_redo=True),
    'fixed_no_trim': case(configured(False, trim=False) + PICKS),
    'zero_joined': case(configured(False, radius=0., join=True) + PICKS),
    'reverse_pick_order': case(configured(False, join=True) + list(reversed(PICKS))),
    'options_after_first': case(configured(False) + [PICKS[0], 'Radius', 1., 'Join=Yes', PICKS[1]]),
    'cancel_second': case(configured(False) + [PICKS[0], 'Cancel'], pending_redo=True),
    'cancel_radius_setting': case(['Radius', .875, 'Cancel'], pending_redo=True),
    'remember_cancelled_radius': case(['Cancel'], pending_redo=True),
    'extended_arcs': case(configured(False, radius=.2, join=True) + ['ExtendArcsBy=Arc', pick(0, [0., 1., 0.]), pick(1, [-.5, 2.1, 0.])], curves=[dict(type='arc', points=[[1., 0., 0.], [2.**-.5, 2.**-.5, 0.], [0., 1., 0.]]), dict(type='line', start=[-.5, 2., 0.], end=[-.5, 3., 0.])]),
}
CASES.update(HOLDOUTS)


def explicit(dynamic=False, radius=.5, join=False, trim=True):
    # Join is unavailable with Trim=No. Restore Trim before assigning Join.
    return ['Radius', radius, 'Trim=Yes', 'Join=' + ('Yes' if join else 'No'),
            'Trim=' + ('Yes' if trim else 'No'), 'Dynamic=' + ('Yes' if dynamic else 'No')]


VERIFICATION = {
    'numeric_then_enter': case(explicit(True, join=True) + PICKS + [1.25, shot('numeric'), 'Enter']),
    'measured_then_enter': case(explicit(True) + PICKS + [[0., 0., 0.], dict(hover=[0., 1.5, 0.]), shot('distance'), [0., 1.5, 0.], shot('measured'), 'Enter']),
    'repeated_numbers': case(explicit(True) + PICKS + [1.25, .75, shot('repeated'), 'Enter']),
    'zero_joined_confirmed': case(explicit(False, radius=0., join=True) + PICKS),
    'zero_parts': case(explicit(False, radius=0.) + PICKS),
    'zero_untrimmed_cancel': case(explicit(True, radius=0., trim=False) + PICKS + ['Cancel'], pending_redo=True),
    'cancel_changed_preview': case(explicit(True, join=True) + PICKS + [1.5, shot('changed_cancel'), 'Cancel'], pending_redo=True),
    'remember_preview_radius': case(['Cancel'], pending_redo=True),
    'spatial_joined': case(explicit(False, radius=.4, join=True) + [pick(0, [3.6, 0., 1.8]), pick(1, [4., .4, 2.])], curves=[dict(type='line', start=[0., 0., 0.], end=[4., 0., 2.]), dict(type='line', start=[4., 0., 2.], end=[4., 4., 2.])]),
    'curved_contact': case(explicit(True, radius=.2) + [pick(0, [1.95, .95, 0.]), pick(1, [2., 1.15, 0.]), 'Enter'], curves=[dict(type='nurbs', degree=2, control_points=[dict(point=p, weight=1.) for p in [[0., 0., 0.], [1., 0., 0.], [2., 1., 0.]]], knots=[0., 0., 0., 1., 1., 1.]), dict(type='line', start=[2., 1., 0.], end=[2., 5., 0.])]),
}
CASES.update(VERIFICATION)
FINAL = {k: v for k, v in VERIFICATION.items() if k != 'curved_contact'}
FINAL['curved_contact_separated'] = case(explicit(True, radius=.2) + [pick(0, [1.5, .5625, 0.]), pick(1, [2., 1.9, 0.]), 'Enter'], curves=VERIFICATION['curved_contact']['curves'])
CASES.update(FINAL)

if __package__:
    from .fillet_closed_cases import pilot, holdouts
else:
    from fillet_closed_cases import pilot, holdouts
CLOSED_PILOT = pilot(case, explicit, pick)
CASES.update(CLOSED_PILOT)
CLOSED_HOLDOUTS = holdouts(case, explicit, pick)
CASES.update(CLOSED_HOLDOUTS)
from copy import deepcopy
CLOSED_FINAL = deepcopy(CLOSED_HOLDOUTS)
CLOSED_FINAL['circle_line_spatial'] = dict(CLOSED_FINAL['circle_line_spatial'], view='Top')
# Keep the failed request immutable and give the corrected spatial pick a new ID.
CLOSED_FINAL['circle_line_spatial_top'] = CLOSED_FINAL.pop('circle_line_spatial')
CASES.update(CLOSED_FINAL)

if __package__:
    from .fillet_self_cases import pilot as self_pilot, discovery as self_discovery, holdouts as self_holdouts, branches as self_branches, limits as self_limits
else:
    from fillet_self_cases import pilot as self_pilot, discovery as self_discovery, holdouts as self_holdouts, branches as self_branches, limits as self_limits
SELF_PILOT = self_pilot(case, explicit, pick, shot)
CASES.update(SELF_PILOT)
SELF_DISCOVERY = self_discovery(case, explicit, pick)
CASES.update(SELF_DISCOVERY)
SELF_HOLDOUTS = self_holdouts(case, explicit, pick)
CASES.update(SELF_HOLDOUTS)
SELF_BRANCHES = self_branches(case, explicit, pick)
CASES.update(SELF_BRANCHES)
SELF_LIMITS = self_limits(case, explicit, pick)
CASES.update(SELF_LIMITS)


def validate(op):
    if not isinstance(op, dict) or set(op) != {'op', 'id', 'case'} or op.get('op') != 'fillet_command' or not isinstance(op.get('case'), str) or op.get('case') not in CASES or op.get('id') != op['case']:
        raise ValueError('invalid prescribed Fillet getter recipe')


def request(names=None):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6),
                operations=[dict(op='fillet_command', id=k, case=k) for k in (PILOT if names is None else names)])
