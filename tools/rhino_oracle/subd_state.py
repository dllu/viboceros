"""Public SubD control identities, colors, selected faces and scene snapshots."""


def geometry_value(g,host,value=None):
    from subd_box_probe import subd_value
    from subd_edit_scene import Scene
    if value is None:value=subd_value(g,host)
    value.update(vertex_ids=[int(v.Id)for v in Scene.vertices(g)],edge_ids=[int(e.Id)for e in g.Edges],
        face_ids=[int(f.Id)for f in g.Faces],
        face_colors=[None if f.PerFaceColor.IsEmpty else [int(f.PerFaceColor.R),int(f.PerFaceColor.G),int(f.PerFaceColor.B),int(f.PerFaceColor.A)]for f in g.Faces])
    return value


def snapshot(scene):
    rows=scene.snapshot()
    for row,obj in zip(rows,sorted(list(scene.doc.Objects),key=lambda o:o.RuntimeSerialNumber)):
        if 'subd'not in row:continue
        row['subd']=geometry_value(obj.Geometry,scene.host,row['subd'])
        row['selected_faces']=[int(f.Id)for f in obj.Geometry.Faces if obj.IsSubObjectSelected(f.ComponentIndex())]
    return rows
