"""First-pick depth, translated views, and point-local Undo holdouts."""
from copy import deepcopy
if __package__:
    from .subdivide_probe import source
else:
    from subdivide_probe import source
CASES = {}
for kind in ['mesh', 'subd']:
    vertices = [[0., 0., 0.], [4., 0., 0.], [4., 3., 1.2], [0., 3., .4]]
    low, high, bottom = [0., .75, .1], [0., 2.25, .3], [2., 0., 0.]
    for name, actions, offset, shift in [
        ('low', [dict(click=low), 'Enter'], 0., 0.),
        ('high', [dict(click=high), 'Enter'], 0., 0.),
        ('target_plus', [dict(click=low), 'Enter'], 5., 0.),
        ('undo_repick', [dict(click=low), 'Undo', dict(click=high), 'Enter'], 0., 0.),
        ('translated', [dict(click=low), 'Enter'], 0., 10.),
        ('second_point', [dict(click=bottom), dict(click=low), 'Enter'], 0., 0.),
    ]:
        points = [[x, y, z+shift] for x, y, z in vertices]
        actions = deepcopy(actions)
        for action in actions:
            if isinstance(action, dict):action['click'][2] += shift
        CASES['insert_point_depth_'+kind+'_'+name] = dict(
            command='InsertPoint', sources=[source(kind, vertices=points, faces=[[0, 1, 2, 3]])],
            whole=True, preselected=True, live=True, mesh_channels=False,
            points=[a['click'] for a in actions if isinstance(a, dict)], actions=actions,
            view_target_offset_z=offset)

WITNESSES = {}
for kind in ['mesh', 'subd']:
    for name, base in [('inside', 'low'), ('inside_target_plus', 'target_plus'),
                       ('inside_translated', 'translated'), ('second_inside', 'second_point')]:
        recipe = deepcopy(CASES['insert_point_depth_'+kind+'_'+base])
        for action in recipe['actions']:
            if isinstance(action, dict) and action['click'][0] == 0.:
                action['click'][0] = .025
        recipe['points'] = [a['click'] for a in recipe['actions'] if isinstance(a, dict)]
        recipe['capture_pick_hint'] = True
        WITNESSES['insert_point_depth_v2_'+kind+'_'+name] = recipe
