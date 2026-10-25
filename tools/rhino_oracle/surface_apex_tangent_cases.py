"""Independent closed boundaries with geometric and parametric continuity joins."""
import copy
import math


def curve(points, weights=None, spans=None, degree=2):
    weights = weights or [1.]*len(points)
    count = (len(points)-1)//degree
    spans = spans or [1.]*count
    ends = [0.]
    for length in spans:
        ends.append(ends[-1]+length)
    knots = [ends[0]]*(degree+1)+[k for t in ends[1:-1] for k in [t]*degree]+[ends[-1]]*(degree+1)
    return dict(degree=degree, knots=knots,
                control_points=[dict(point=list(p), weight=w) for p, w in zip(points, weights)])


def sources():
    w = math.sqrt(.5)
    points = [[-2.,1.,0.],[-3.,1.,0.],[-3.,0.,0.],[-3.,-1.,0.],[-2.,-1.,0.],
              [0.,-1.,0.],[2.,-1.,0.],[3.,-1.,0.],[3.,0.,0.],[3.,1.,0.],
              [2.,1.,0.],[0.,1.,0.],[-2.,1.,0.]]
    weights = [1.,w,1.,w,1.,1.,1.,w,1.,w,1.,1.,1.]
    capsule = curve(points, weights)
    rotated = points[2:-1]+points[:3]
    rotated_weights = weights[2:-1]+weights[:3]
    circle_points = [[1.,0.,0.],[1.,1.,0.],[0.,1.,0.],[-1.,1.,0.],[-1.,0.,0.],
                     [-1.,-1.,0.],[0.,-1.,0.],[1.,-1.,0.],[1.,0.,0.]]
    circle_weights = [1.,w,1.,w,1.,w,1.,w,1.]
    profiles = [
        ('capsule', capsule),
        ('capsule_c1', curve(points, weights, [1.,1.,2.*math.sqrt(2.),1.,1.,2.*math.sqrt(2.)])),
        ('capsule_seam', curve(rotated, rotated_weights)),
        ('circle', curve(circle_points, circle_weights)),
        ('circle_speed', curve(circle_points, circle_weights, [1.,2.,3.,4.])),
        ('corners', curve([[-2.,-1.,0.],[2.,-1.,0.],[2.,1.,0.],[-2.,1.,0.],[-2.,-1.,0.]], degree=1)),
        ('hole', capsule), ('tilted', capsule),
    ]
    carrier = dict(degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
                   knots_u=[-4.,-4.,4.,4.], knots_v=[-2.,-2.,2.,2.],
                   control_points=[dict(point=list(p), weight=1.) for p in
                                   [(-4.,-2.,0.),(4.,-2.,0.),(-4.,2.,0.),(4.,2.,0.)]])
    result = []
    for name, profile in profiles:
        boundary = dict(curve=copy.deepcopy(profile), parameter_curve=copy.deepcopy(profile))
        source = dict(type='brep', surface=copy.deepcopy(carrier), boundaries=[boundary],
                      interior_uv=[1.5,0.] if name == 'hole' else [0.,0.])
        if name == 'hole':
            hole = curve([[.3*x,.3*y,z] for x,y,z in reversed(circle_points)], circle_weights)
            source['boundaries'].append(dict(curve=hole, parameter_curve=copy.deepcopy(hole)))
        apex = [1.,.5,6.]
        if name == 'tilted':
            def transform(p):
                x,y,z = p
                return [10.+x,20.+.6*y-.8*z,30.+.8*y+.6*z]
            for cp in source['surface']['control_points']+source['boundaries'][0]['curve']['control_points']:
                cp['point'] = transform(cp['point'])
            apex = transform(apex)
        result.append((name, source, apex))
    return result


def request():
    operations = []
    for name, source, apex in sources():
        for split in [False, True]:
            for solid in [False, True]:
                for delete in [False, True]:
                    for preselect in [False, True]:
                        operations.append(dict(op='surface_apex_command',
                            id='apex-tangent-'+name+'-{}{}{}{}'.format(int(split),int(solid),int(delete),int(preselect)),
                            source=copy.deepcopy(source), face=0, apex=apex, solid=solid,
                            delete_input=delete, preselect=preselect, split_at_tangents=split))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-8), operations=operations)


def angle_request():
    """Holdouts straddling the native curve continuity angle threshold."""
    return _angle_request([0.,.05,.5,1.,1.01,2.], 'angle')


def threshold_request():
    return _angle_request([.099,.1,.101,.25], 'threshold')


def detail_request():
    q = _angle_request([.05,.099,.1,.101,.25], 'detail')
    q['operations'] = [op for op in q['operations'] if not op['solid']]
    for op in q['operations']:
        op['surface_details'] = True
    return q


def _angle_request(angles, label):
    q = request()
    base = q['operations'][0]
    q['operations'] = []
    for index, degrees in enumerate(angles):
        for split in [False, True]:
            for solid in [False, True]:
                op = copy.deepcopy(base)
                op.update(id='apex-tangent-{}-{}-{}{}'.format(label,index,int(split),int(solid)),
                          split_at_tangents=split, solid=solid)
                delta = 2.*math.tan(math.radians(degrees))
                for field in ['curve', 'parameter_curve']:
                    op['source']['boundaries'][0][field]['control_points'][11]['point'][1] += delta
                q['operations'].append(op)
    return q
