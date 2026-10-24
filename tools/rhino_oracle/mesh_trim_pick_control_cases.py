"""Repeat vertical-region controls with native input-location tracing."""
from copy import deepcopy


def cases(controls):
    result = {}
    for suffix in ['curve_direct_quad', 'curve_only_quad', 'mesh_only_quad', 'horizontal_only_quad']:
        name = 'mesh_trim_pick_controls_' + suffix
        value = deepcopy(controls['mesh_trim_curve_controls_' + suffix])
        value.update(capture_id=name, trace_picks=True)
        result[name] = value
    return result
