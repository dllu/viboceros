"""Direct physical getters, preselection, and independent native mesh partitions."""
from copy import deepcopy


def cases(controls, mouse, wall):
    result = {}
    for name, source, cutters in [
            ('mesh_only_sdk', 'mesh_only_quad', [1]),
            ('horizontal_only_sdk', 'horizontal_only_quad', [0]),
            ('two_meshes_sdk', 'mesh_after_quad', [0, 1])]:
        identifier = 'mesh_trim_getter_controls_' + name
        value = deepcopy(controls['mesh_trim_curve_controls_' + source])
        value.update(capture_id=identifier, first=cutters, trim_picks=[[1., 1., .5]],
                     trim_targets=[2], direct_getter=True, trace_picks=True,
                     trace_objects=False, sdk_cutters=cutters)
        if source == 'mesh_after_quad':
            value['sources'][1] = deepcopy(wall)
        result[identifier] = value
    for name, source in [('sheet_pre', 'sheet_left_pre'), ('box_pre', 'box_left_post')]:
        identifier = 'mesh_trim_getter_controls_' + name
        value = deepcopy(mouse['mesh_trim_mouse_' + source])
        value.update(capture_id=identifier, preselect=True, direct_getter=True,
                     trace_picks=True, trace_objects=False)
        result[identifier] = value
    return result


def plane_cases(controls, xy, moved):
    result = {}
    for name in ['three_point', 'world_yz']:
        identifier = 'mesh_trim_plane_getters_' + name
        value = deepcopy(controls['mesh_trim_controls_plane'])
        value.update(capture_id=identifier, trace_picks=True, trace_objects=False)
        if name == 'world_yz':
            value.update(sources=[moved(xy, [-2., 0., 0.])], trim_picks=[[-1., 1., 0.]],
                         cutting_arguments='_IP _WorldYZ', infinite_plane='WorldYZ')
        result[identifier] = value
    return result
