"""Public Cap with component identities and full per-stage SubD archives."""
try:
    from .subd_cap_regions_cases import CASES,validate
except (ImportError,ValueError):
    from subd_cap_regions_cases import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    import subd_box_probe
    from crease_probe import run_scene
    original=subd_box_probe.subd_value
    def observe(g,host):
        value=original(g,host)
        vertices=[];v=g.Vertices.First
        while v is not None:vertices.append(v);v=v.Next
        value.update(vertex_ids=[int(v.Id)for v in vertices],edge_ids=[int(e.Id)for e in g.Edges],
            face_ids=[int(f.Id)for f in g.Faces],
            face_colors=[None if f.PerFaceColor.IsEmpty else[int(f.PerFaceColor.R),int(f.PerFaceColor.G),int(f.PerFaceColor.B),int(f.PerFaceColor.A)]for f in g.Faces],
            texture_coordinate_type=int(g.TextureCoordinateType))
        uses={}
        for face in value['faces']:
            for i,a in enumerate(face):
                key=tuple(sorted([a,face[(i+1)%len(face)]]));uses[key]=uses.get(key,0)+1
        value['edge_face_counts']=[uses.get(tuple(sorted(edge)),0)for edge in value['edges']]
        R=host['Rhino'];S=host['System'];model=R.FileIO.File3dm()
        try:
            layer=model.Layers.AddLayer('Default',S.Drawing.Color.Black)
            attrs=R.DocObjects.ObjectAttributes();attrs.LayerIndex=layer
            model.Objects.Add(g,attrs)
            value['geometry_3dm']=S.Convert.ToBase64String(model.ToByteArray())
        finally:model.Dispose()
        return value
    subd_box_probe.subd_value=observe
    try:return run_scene(CASES[op['case']],host,iterations,tolerance)
    finally:subd_box_probe.subd_value=original
