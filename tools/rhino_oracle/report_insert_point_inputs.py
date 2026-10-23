"""Independent physical input witnesses for source-edge insertion replays."""
import math


def validate_depth(click, required=False, diagnostic=False):
    def vector(key, optional=False):
        value = click.get(key)
        if optional and value is None:
            return None
        if (not isinstance(value, list) or len(value) != 3
                or any(type(x) not in (int, float) or not math.isfinite(x) for x in value)):
            raise ValueError('invalid independent insertion '+key)
        return value

    if not required and 'view_target' not in click:
        return
    target, normal = vector('view_target'), vector('camera_direction')
    if abs(sum(x*x for x in normal)-1.) > 1e-9:
        raise ValueError('invalid insertion camera direction')
    a, b = click['ray']
    direction = [y-x for x, y in zip(a, b)]
    denominator = sum(x*y for x, y in zip(direction, normal))
    if denominator == 0.:
        raise ValueError('insertion ray parallel to target plane')
    parameter = sum((t-x)*n for t, x, n in zip(target, a, normal))/denominator
    point = [x+parameter*d for x, d in zip(a, direction)]
    if diagnostic:
        plane = vector('view_target_point')
        if math.dist(point, plane) > 1e-8:
            raise ValueError('independent insertion target-plane witness differs')
        # The SDK ray hit is a calibration of its shorter-diagonal policy, not
        # the native getter's depth hint. Never feed it back into the replay.
        vector('ray_hit_point', optional=True)
        if 'ray_hit_point' not in click:
            raise ValueError('missing SDK triangulation diagnostic')


def witnessed(capture, family):
    result = {}
    for row in capture(family)['results']:
        value = row.get('value')
        if value is None or 'Unknown command' in value['history']:
            raise ValueError('missing native insertion workflow')
        traces = value['input_trace']
        if len(traces) != 1:
            raise ValueError('unexpected insertion input trace')
        trace = traces[0]
        actions = value['input']['actions']
        clicks = trace.get('clicks', [])
        wanted = [i for i, a in enumerate(actions) if isinstance(a, dict)]
        if (trace.get('errors') or trace.get('completed') != len(actions)
                or trace.get('expected') != len(actions)
                or [c['action'] for c in clicks] != wanted):
            raise ValueError('incomplete native physical insertion input')
        for click in clicks:
            if (click.get('click_acknowledged') is not True
                    or click.get('motion_acknowledged') is not True
                    or len(click.get('ray', [])) != 2
                    or any(len(p) != 3 for p in click['ray'])
                    or len(click.get('projected_point', [])) != 3
                    or any(type(x) not in (int, float) or not math.isfinite(x)
                           for p in click['ray']+[click['projected_point']] for x in p)):
                raise ValueError('missing native physical point witness')
        for click in clicks:
            validate_depth(click, required='pick_depth' in family or family.startswith('insert_point_spatial_'), diagnostic=family.endswith('depth_v2'))
        result[row['id']] = value
    return result
