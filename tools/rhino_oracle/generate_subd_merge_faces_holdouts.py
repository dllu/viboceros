"""Disambiguate SubD cleanup, mixed selections and surviving edge properties."""
from copy import deepcopy
from .capture_family import capture
from .generate_subd_merge_faces import request as base


def request():
    original = {row['id']: row for row in base()['operations']}
    rows = []
    def add(name, source, components=None, **options):
        row = deepcopy(original[source])
        row.update(id=name, **options)
        if components is not None:
            row['components'] = components
        rows.append(row)
    face = lambda *indices: [dict(kind='face', index=i) for i in indices]
    edge = lambda a, b: [dict(kind='edge', vertices=[a, b])]
    vertex = lambda i: [dict(kind='vertex', index=i)]
    twisted = deepcopy(original['strip-faces']['vertices'])
    twisted[5][2] = 1.25
    for kind, picks in [('faces', face(0, 1)), ('edge', edge(1, 2)), ('vertex', vertex(1))]:
        add('twisted-' + kind, 'strip-faces', picks, vertices=twisted)
    for source, shared in [('strip-faces', [1, 2]), ('tri-faces', [0, 2])]:
        for suffix, corners in [('first', shared[:1]), ('second', shared[1:]), ('both', shared),
                                ('all', list(range(len(original[source]['vertices']))))]:
            add(source + '-corner-' + suffix, source, corners=corners)
    for source in ['tri-faces', 'strip-faces', 'fan-faces', 'cube-faces']:
        add('reversed-' + source, source, faces=[list(reversed(f)) for f in original[source]['faces']])
    for name, picks in [('faces-overlap-edge', face(0, 1) + edge(4, 5)),
                        ('edge-overlap-faces', edge(4, 5) + face(0, 1)),
                        ('face-between-edges', edge(1, 2) + face(1) + edge(4, 5)),
                        ('vertex-next-edge', vertex(1) + edge(4, 5)),
                        ('edge-next-vertex', edge(1, 2) + vertex(4)),
                        ('faces-overlap-vertex', face(0, 1) + vertex(4)),
                        ('vertex-overlap-faces', vertex(4) + face(0, 1)),
                        ('face-overlap-vertex', face(0) + vertex(1))]:
        add(name, 'three-faces', picks)
    for name, pair in [('removed-end', [0, 4]), ('other-end', [0, 3]), ('unaffected', [4, 7])]:
        for weight in [25, 75]:
            add('side-soft-' + name + '-' + str(weight), 'cube-faces',
                soft=dict(weight=weight, edges=[pair]))
    grid = dict(vertices=[[x, y, .2*x*y] for y in range(3) for x in range(3)],
                faces=[[3*y+x, 3*y+x+1, 3*y+x+4, 3*y+x+3] for y in range(2) for x in range(2)])
    add('quad-grid-faces', 'strip-faces', face(*range(4)), **grid)
    add('quad-grid-vertex', 'strip-faces', vertex(4), **grid)
    add('quad-grid-two-faces', 'strip-faces', face(0, 1), **grid)
    query = base()
    query['operations'] = rows
    return query


if __name__ == '__main__':
    capture('subd_merge_faces_holdouts', request(), ['generate_subd_merge_faces_holdouts.py',
            'generate_subd_merge_faces.py', 'subd_merge_faces_probe.py', 'subd_edit_scene.py',
            'subd_box_probe.py', 'hide_pt_input.py', 'number_token.py', 'join_probe.py',
            'merge_edges_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSubDMergeHoldouts20261029', timeout=600)
