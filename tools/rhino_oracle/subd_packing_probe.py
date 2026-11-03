"""Public PackFaces and ToBrep controls with complete paired geometry."""
if __package__:
    from .subd_packing_cases import cases as base_cases,source as base_source,mode_cases
    from .subd_packing_holdouts import cases as holdout_cases,source as holdout_source,SCENES as HOLDOUTS
    from .subd_to_nurbs_probe import make_source
    from .subd_box_probe import subd_value
else:
    from subd_packing_cases import cases as base_cases,source as base_source,mode_cases
    from subd_packing_holdouts import cases as holdout_cases,source as holdout_source,SCENES as HOLDOUTS
    from subd_to_nurbs_probe import make_source
    from subd_box_probe import subd_value

def validate(op):
    if not isinstance(op,dict) or op not in base_cases()+holdout_cases()+mode_cases() or type(op.get('mode')) is not int:
        raise ValueError('fixed SubD packing recipe required')

def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];G=R.Geometry;S=host['System']
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    recipe=holdout_source(op['scene']) if op['scene'] in HOLDOUTS else base_source(op['scene'])
    # Mesh creation welds coincident inputs. Establish two disconnected nets
    # first, then move their existing vertices through the public SDK.
    subd=make_source(base_source('two_grids') if op['scene']=='coincident_grids' else recipe,host)
    try:
        if op['scene']=='coincident_grids':
            vertex=subd.Vertices.First;i=0
            while vertex is not None:
                if i>=len(recipe['points'])//2 and not vertex.SetControlNetPoint(G.Point3d(*recipe['points'][i]),False):raise ValueError('cannot move owned coincident net')
                vertex=vertex.Next;i+=1
            subd.ClearEvaluationCache()
        if op['scene'].startswith('subdivided_box') and not subd.Subdivide(2 if op['scene'].endswith('2') else 1):raise ValueError('cannot subdivide owned box')
        packs=int(subd.PackFaces());before=subd_value(subd,host);crc=int(subd.DataCRC(0))
        model=R.FileIO.File3dm()
        try:model.Objects.AddSubD(subd);archive=[int(v) for v in model.ToByteArray()]
        finally:model.Dispose()
        outputs=[]
        for packed in (False,True):
            options=G.SubDToBrepOptions(packed,S.Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,op['mode']));brep=None
            try:
                brep=subd.ToBrep(options)
                if brep is None or not brep.IsValid:raise ValueError('native packing conversion failed')
                outputs.append(dict(valid=True,solid=bool(brep.IsSolid),pack_ids=[int(f.PackId) for f in brep.Faces],
                    geometry=host['_interchange_brep_record'](brep)))
            finally:
                if brep is not None:brep.Dispose()
                options.Dispose()
        if int(subd.DataCRC(0))!=crc:raise ValueError('native packing conversion edited source')
        return dict(input=recipe,before=before,archive=archive,pack_count=packs,source_unchanged=True,
                    unpacked=outputs[0],output=outputs[1]),0
    finally:subd.Dispose()
