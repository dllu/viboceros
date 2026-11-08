"""Observe Shell using authored opening points and public face preselection."""
if __package__:
    from .shell_command import CASES, validate
else:
    from shell_command import CASES, validate


def run(op, host, iterations, tolerance):
    validate(op)
    from line_scene_probe import run_case
    recipe = CASES[op['case']]
    R = host['Rhino']; G = R.Geometry; doc = R.RhinoDoc.ActiveDoc
    selected_faces = []

    def setup(ids):
        doc.Objects.UnselectAll()
        for key, points in zip(ids, recipe['openings']):
            obj = doc.Objects.FindId(key)
            faces = set()
            for xyz in points:
                point = G.Point3d(*xyz)
                matches = []
                for i, face in enumerate(obj.Geometry.Faces):
                    ok, u, v = face.ClosestPoint(point)
                    if (ok and face.PointAt(u, v).DistanceTo(point) <= tolerance['absolute']
                            and face.IsPointOnFace(u, v) == G.PointFaceRelation.Interior):
                        matches.append(i)
                if len(matches) != 1:
                    raise ValueError('opening point must identify one face interior')
                faces.add(matches[0])
            for i in sorted(faces):
                component = G.ComponentIndex(G.ComponentIndexType.BrepFace, i)
                if not obj.SelectSubObject(component, True, True, False):
                    raise ValueError('owned Shell face preselection failed')
            selected_faces.append(sorted(faces))

    value, elapsed = run_case(op, host, iterations, tolerance, recipe, 'Shell', False,
                              source_setup=setup, capture_undo=True)
    value['prescribed_case'] = recipe
    value['selected_opening_faces'] = selected_faces
    return value, elapsed
