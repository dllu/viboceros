"""Source-only controls distinguishing open-surface first-moment conventions."""
import copy
import json
from .volume_centroid import tetrahedron


def patch(points):
    return dict(type="surface", degree_u=1, degree_v=1,
                control_point_count_u=2, control_point_count_v=2,
                control_points=[dict(point=p, weight=1.) for p in points],
                knots_u=[0,0,1,1], knots_v=[0,0,1,1])


def transformed(source, transform):
    result = copy.deepcopy(source)
    if result['type'] == 'surface':
        for c in result['control_points']: c['point'] = transform(*c['point'])
    else: result['vertices'] = [transform(*p) for p in result['vertices']]
    return result


def rotate(x,y,z):
    # A genuine non-axis rotation, not just an axis permutation. Binary64
    # controls are themselves the exact inputs to the independent reference.
    a,b = (3*x-4*y)/5, (4*x+3*y)/5
    return [a,(3*b-4*z)/5,(4*b+3*z)/5]


def request():
    operations = []
    def add(name, sources):
        operations.append(dict(op='volume_centroid_command', id=name,
            sources=copy.deepcopy(sources), collection_api=True, open_confirmation='yes'))
    base = patch([[0,0,0],[4,0,0],[0,3,0],[4,3,2]])
    generic = patch([[1,-2,3],[5,1,-1],[-3,4,2],[7,6,8]])
    for name, source in [('bilinear',base),('spatial',generic)]:
        for label, fn in [('original',lambda x,y,z:[x,y,z]), ('rotated',rotate),
                          ('sheared',lambda x,y,z:[x+y, y+z, z+x/2]),
                          ('translated',lambda x,y,z:[x+10,y-20,z+30])]:
            add(name+'-'+label,[transformed(source,fn)])
        # Closed opposite mesh shells cancel exactly as distributions, but
        # alter the common bounding reference used by the open surface.
        for offset in [-10., 10., 30.]:
            anchor = transformed(tetrahedron(scale=2.),lambda x,y,z:[x+offset,y-5,z+7])
            reversed_anchor = copy.deepcopy(anchor)
            reversed_anchor['faces'] = [f[::-1] for f in anchor['faces']]
            add(name+'-reference-'+str(int(offset)),[source,anchor,reversed_anchor])
    add('two-surfaces',[base,transformed(generic,lambda x,y,z:[x+5,y+4,z+2])])
    add('two-surfaces-reverse-order',list(reversed(operations[-1]['sources'])))
    vertices = [[0,0,0],[4,0,0],[4,3,0],[0,3,0],[0,0,2],[4,0,2],[4,3,2],[0,3,2]]
    faces = [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]
    meshes = [dict(type='mesh',vertices=[vertices[i] for i in f],faces=[[0,1,2,3]]) for f in faces]
    surfaces = [patch([vertices[f[i]] for i in (0,1,3,2)]) for f in faces]
    for label, indices in [('all-surfaces',[]),('all-meshes',list(range(6)))] + [
            ('mesh-face-'+str(i),[i]) for i in range(6)] + [('opposed-meshes',[0,1]),('alternating-meshes',[0,2,4])]:
        sources = [meshes[i] if i in indices else surfaces[i] for i in range(6)]
        add('box-'+label,sources)
        add('rotated-box-'+label,[transformed(s,rotate) for s in sources])
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__=='__main__': print(json.dumps(request(),indent=2,allow_nan=False))
