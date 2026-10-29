"""Capture SubD merge regions, component identities and crease boundaries."""
from copy import deepcopy
from .capture_family import capture


def request():
    tri = dict(vertices=[[0, 0, 0], [2, 0, 0], [2, 2, 0], [0, 2, 0]],
               faces=[[0, 1, 2], [0, 2, 3]])
    strip = dict(vertices=[[0, 0, 0], [2, 0, 0], [2, 2, 0], [0, 2, 0], [4, 0, 0], [4, 2, 0]],
                 faces=[[0, 1, 2, 3], [1, 4, 5, 2]])
    bent = dict(strip, vertices=[[0, 0, 0], [2, 0, 1], [2, 2, 0], [0, 2, -1], [4, 0, 2], [4, 2, 1]])
    fan = dict(vertices=[[0, 0, 0], [2, 0, 0], [0, 2, 0], [-2, 0, 0], [0, -2, 0]],
               faces=[[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]])
    cube = dict(vertices=[[0, 0, 0], [2, 0, 0], [2, 2, 0], [0, 2, 0],
                          [0, 0, 2], [2, 0, 2], [2, 2, 2], [0, 2, 2]],
                faces=[[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4],
                       [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]])
    rows = []
    def add(name, scene, components, **options):
        rows.append(dict(op='subd_merge_faces', id=name, components=components,
                         corners=options.get('corners', []), creases=options.get('creases', []),
                         soft=options.get('soft'), **deepcopy(scene)))
    face = lambda *indices: [dict(kind='face', index=i) for i in indices]
    edge = lambda a, b: [dict(kind='edge', vertices=[a, b])]
    vertex = lambda i: [dict(kind='vertex', index=i)]
    for name, scene, faces, pair, index in [('tri', tri, [0, 1], [0, 2], 0),
            ('strip', strip, [0, 1], [1, 2], 1), ('bent', bent, [0, 1], [1, 2], 1),
            ('fan', fan, [0, 1, 2, 3], [0, 2], 0), ('cube', cube, [0, 2], [0, 1], 0)]:
        add(name + '-faces', scene, face(*faces))
        add(name + '-edge', scene, edge(*pair))
        add(name + '-vertex', scene, vertex(index))
    add('single-face', strip, face(0))
    add('empty-selection', strip, [])
    add('boundary-edge', strip, edge(0, 1))
    add('boundary-vertex', strip, vertex(0))
    add('all-closed-faces', cube, face(*range(6)))
    add('all-closed-edges', cube, [dict(kind='edge', vertices=p) for p in
        [[0, 1], [1, 2], [2, 3], [0, 3], [4, 5], [5, 6], [6, 7], [4, 7],
         [0, 4], [1, 5], [2, 6], [3, 7]]])
    ring = dict(vertices=[[x, y, 0] for y in range(4) for x in range(4)],
                faces=[[4*y+x, 4*y+x+1, 4*y+x+5, 4*y+x+4]
                       for y in range(3) for x in range(3) if (x, y) != (1, 1)])
    add('ring-faces', ring, face(*range(8)))
    add('partial-ring', ring, face(0, 1, 2, 3))
    add('ring-edges', ring, [dict(kind='edge', vertices=p) for p in
        [[1, 5], [2, 6], [4, 5], [6, 7], [8, 9], [10, 11], [9, 13], [10, 14]]])
    separate = dict(vertices=tri['vertices'] + [[10, 0, 0], [12, 0, 0], [12, 2, 0], [10, 2, 0]],
                    faces=tri['faces'] + [[4, 5, 6], [4, 6, 7]])
    add('disjoint-face-pairs', separate, face(0, 1, 2, 3))
    add('disjoint-edge-pairs', separate, edge(0, 2) + edge(4, 6))
    strip3 = dict(vertices=strip['vertices'] + [[6, 0, 0], [6, 2, 0]],
                  faces=strip['faces'] + [[4, 6, 7, 5]])
    add('three-faces', strip3, face(0, 1, 2))
    add('three-faces-reverse-picks', strip3, face(2, 1, 0))
    add('two-edges', strip3, edge(1, 2) + edge(4, 5))
    add('mixed-face-edge', strip3, face(0) + edge(4, 5))
    add('mixed-vertex-face', strip3, vertex(1) + face(2))
    for kind, picks in [('faces', face(0, 1)), ('edge', edge(1, 2)), ('vertex', vertex(1))]:
        add('hard-' + kind, strip, picks, creases=[[1, 2]])
        add('soft-' + kind, strip, picks, soft=dict(weight=50, edges=[[1, 2]]))
    add('hard-cube-faces', cube, face(0, 2), creases=[[0, 1]])
    add('corner-survives', strip, face(0, 1), corners=[0])
    add('side-soft-survives', cube, face(0, 2), soft=dict(weight=50, edges=[[2, 3]]))
    add('soft-cube-edge', cube, edge(0, 1), soft=dict(weight=50, edges=[[0, 1]]))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10), operations=rows)


if __name__ == '__main__':
    capture('subd_merge_faces', request(), ['generate_subd_merge_faces.py', 'subd_merge_faces_probe.py',
            'subd_edit_scene.py', 'subd_box_probe.py', 'hide_pt_input.py', 'number_token.py',
            'join_probe.py', 'merge_edges_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSubDMergeFaces20261029', timeout=600)
