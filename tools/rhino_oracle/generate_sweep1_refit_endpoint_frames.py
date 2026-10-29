"""Endpoint frames with forward, perpendicular and backward tangents."""
import copy
import math
from .capture_family import capture

FAMILY = 'sweep1_refit_endpoint_frames'


def request():
    operations = []
    frames = [('start-'+str(c),c,.5) for c in [-.5,-1e-4,0.,1e-4,.5]]
    frames += [('end-'+str(c),.5,c) for c in [-.5,-1e-4,0.,1e-4,.5]]
    frames += [('both-negative',-.5,-.5),('opposite',-.9,-.2)]
    for label, start, end in frames:
        points = [[0.,0.,0.],[start,math.sqrt(1.-start*start),0.],
            [4.-end,math.sqrt(1.-end*end),0.],[4.,0.,0.]]
        source = dict(degree=3,knots=[0.]*4+[1.]*4,
            control_points=[dict(point=p,weight=1.) for p in points])
        second = copy.deepcopy(source)
        for control in second['control_points']:
            control['point'][2] += 5.
        for count in [1,2]:
            operations.append(dict(op='sweep_refit_sections',
                id='endpoint-frame-{}-count-{}'.format(label,count),
                sections=copy.deepcopy([source,second][:count]),parameters=[-7.,13.][:count],
                refit_tolerance=100.))
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-7,relative=1e-10,angular=1e-10),operations=operations)


if __name__ == '__main__':
    capture(FAMILY, request(), ['generate_sweep1_refit_endpoint_frames.py',
        'sweep_refit_sections_probe.py','geometry_validity_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleEndpointFrames20261029',timeout=300)
