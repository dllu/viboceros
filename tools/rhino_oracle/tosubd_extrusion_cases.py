"""Fixed whole-extrusion conversion and native 3DM source recipes."""
from copy import deepcopy


def recipes(defaults):
    variants = [
        ('rectangle', dict(profile='rectangle', cap=True)),
        ('rectangle_open', dict(profile='rectangle', cap=False)),
        ('rectangle_negative', dict(profile='rectangle', cap=True, height=-2.)),
        ('circle', dict(profile='circle', cap=True)),
        ('circle_open', dict(profile='circle', cap=False)),
        ('open_kink', dict(profile='open_kink', cap=False)),
        ('arc', dict(profile='arc', cap=False)),
        ('tube', dict(profile='circle', cap=True, inner_radius=1.)),
        ('rectangle_miter', dict(profile='rectangle', cap=True,
                                 miter_start=[.2, .3, 1.], miter_end=[-.1, .2, 1.])),
        ('circle_shear', dict(profile='circle', cap=True,
                             transform=[1., .3, .4, 7., 0., 1., .2, -4., 0., 0., 1., 3., 0., 0., 0., 1.])),
    ]
    result = {}
    for name, shape in variants:
        for location in [False, True]:
            for corners in [False, True]:
                key = 'tosubd_extrusion_' + name + '_' + str(int(location)) + str(int(corners))
                result[key] = dict(
                    sources=[dict(kind='extrusion', **deepcopy(shape))], record_extrusion=True,
                    source_3dm=not location and not corners,
                    options=dict(defaults, UseSurface='Location' if location else 'ControlPoints',
                                 SurfaceCorners=corners))
    return result


def pipe_recipes(defaults):
    result = {}
    for cap in [False, True]:
        for location in [False, True]:
            for corners in [False, True]:
                key = 'tosubd_pipe_' + ''.join(str(int(v)) for v in [cap, location, corners])
                result[key] = dict(sources=[dict(kind='extrusion', profile='pipe', cap=cap)],
                    record_extrusion=True, source_3dm=not location and not corners,
                    options=dict(defaults, UseSurface='Location' if location else 'ControlPoints', SurfaceCorners=corners))
    result['tosubd_pipe_profile_frame_diagnostic'] = dict(
        sources=[dict(kind='extrusion', profile='circle', cap=True, inner_radius=1.)],
        record_extrusion=True, options=deepcopy(defaults))
    return result


def conversion_edge_recipes(defaults):
    result = {}
    for bottom in [False, True]:
        for location in [False, True]:
            key = 'tosubd_one_cap_' + str(int(bottom)) + str(int(location))
            result[key] = dict(sources=[dict(kind='extrusion', profile='cylinder',
                cap_bottom=bottom, cap_top=not bottom)], source_3dm=not location,
                record_extrusion=True, options=dict(defaults, UseSurface='Location' if location else 'ControlPoints'))
    for profile in ['rectangle', 'circle', 'pipe']:
        for location in [False, True]:
            key = 'tosubd_transposed_' + profile + '_' + str(int(location))
            result[key] = dict(sources=[dict(kind='extrusion', profile=profile, cap=True, transpose=True)],
                source_3dm=not location, record_extrusion=True,
                options=dict(defaults, UseSurface='Location' if location else 'ControlPoints'))
    for degree, xs, zs, knots in [(1,[0.,1.,2.],[0.,0.,1.],[0.,0.,1.,2.,2.]),
            (2,[0.,.5,1.,1.5,2.],[0.,0.,0.,.5,1.],[0.,0.,0.,1.,1.,2.,2.,2.])]:
        source = dict(kind='surface', degree_u=degree, degree_v=1,
            control_point_count_u=len(xs), control_point_count_v=2, knots_u=knots,
            knots_v=[0.,0.,1.,1.], control_points=[dict(point=[x,float(v),z],weight=1.)for v in range(2)for x,z in zip(xs,zs)])
        for location in [False,True]:
            for corners in [False,True]:
                key = 'tosubd_sdk_kink_' + ''.join(str(int(v))for v in [degree,location,corners])
                result[key] = dict(sources=[deepcopy(source)], sdk=True,
                    options=dict(defaults, UseSurface='Location' if location else 'ControlPoints', SurfaceCorners=corners))
    return result
