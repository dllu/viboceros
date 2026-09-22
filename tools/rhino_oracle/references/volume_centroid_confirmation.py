"""Source-only open-volume warning controls; never reads observed targets."""
import copy
import json

from .volume_centroid import tetrahedron


def request():
    open_tetra = tetrahedron()
    open_tetra["faces"].pop(0)
    unoriented = tetrahedron()
    unoriented["faces"][0].reverse()
    translated = copy.deepcopy(open_tetra)
    translated["vertices"] = [[x+1e6, y-2e6, z+3e6] for x, y, z in translated["vertices"]]
    quad = dict(type="mesh", vertices=[[0,0,0],[4,0,0],[4,3,2],[0,3,0]], faces=[[0,1,2,3]])
    surface = dict(type="surface", degree_u=1, degree_v=1,
                   control_point_count_u=2, control_point_count_v=2,
                   control_points=[dict(point=p, weight=1.) for p in [[0,0,0],[4,0,0],[0,3,0],[4,3,2]]],
                   knots_u=[0,0,1,1], knots_v=[0,0,1,1])
    operations = []
    def add(name, sources, choice, preselect=True):
        operations.append(dict(op="volume_centroid_command", id=name, sources=copy.deepcopy(sources),
                               open_confirmation=choice, preselect=preselect))
    for name, source in [("open-tetra", open_tetra), ("unoriented-tetra", unoriented),
                         ("closed-tetra", tetrahedron()), ("translated-open-tetra", translated),
                         ("open-quad", quad), ("open-surface", surface)]:
        for preselect in (True, False):
            for choice in ("yes", "no", "escape"):
                add("%s-%s-%s" % (name, "pre" if preselect else "post", choice), [source], choice, preselect)
    # Face objects jointly enclose a box, without joining their topology.
    vertices = [[0,0,0],[4,0,0],[4,3,0],[0,3,0],[0,0,2],[4,0,2],[4,3,2],[0,3,2]]
    faces = [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]
    meshes = [dict(type="mesh", vertices=[vertices[i] for i in face], faces=[[0,1,2,3]]) for face in faces]
    surfaces = []
    for face in faces:
        patch = copy.deepcopy(surface)
        patch["control_points"] = [dict(point=vertices[face[i]], weight=1.) for i in (0,1,3,2)]
        surfaces.append(patch)
    for name, sources in [("unjoined-box-meshes", meshes), ("unjoined-box-surfaces", surfaces)]:
        for choice in ("yes", "no", "escape"):
            add(name+"-"+choice, sources, choice)
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__": print(json.dumps(request(), indent=2, allow_nan=False))
