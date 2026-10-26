"""Separate coordinate-frame effects from profile-count preparation in Refit."""
from .capture_family import capture
from .generate_loft_refit_sets import request as sets


def request():
    query = sets()
    for op in query['operations']:
        op['id'] = op['id'].replace('loft-refit-sets-', 'loft-refit-frames-')
        op.pop('inspect_refit_sets')
        op['inspect_refit_frames'] = True
    return query


if __name__ == '__main__':
    capture('loft_refit_frames', request(), ['generate_loft_refit_frames.py',
        'generate_loft_refit_sets.py', 'generate_loft_refit_policy.py', 'generate_loft_refit.py',
        'generate_loft_rebuild.py', 'fixtures/loft.json', 'loft_refit_frame_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'], 'VibocerosOracleLoftRefitFrames20261025')
