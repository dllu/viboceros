"""Native SubDCrease inputs followed by immutable public ToBrep queries."""
if __package__:
    from .subd_soft_nurbs_cases import validate,source
    from .subd_to_nurbs_probe import make_source
    from .subd_box_probe import subd_value
    from .join_probe import observe_command
else:
    from subd_soft_nurbs_cases import validate,source
    from subd_to_nurbs_probe import make_source
    from subd_box_probe import subd_value
    from join_probe import observe_command

def run(op,host,iterations,tolerance):
    validate(op);R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    if 'asset' in op:
        import os
        model=R.FileIO.File3dm.Read(os.path.join(os.path.dirname(__file__),'fixtures','subd_soft_nurbs_inputs',op['asset']+'.3dm'))
        if model is None:raise ValueError('cannot read fixed native soft-crease input')
        try:
            inputs=[o.Geometry for o in model.Objects if isinstance(o.Geometry,G.SubD) and o.Attributes.Name=='Input 0']
            if len(inputs)!=1:raise ValueError('ambiguous fixed native soft-crease input')
            owned=inputs[0].Duplicate()
        finally:model.Dispose()
        try:return convert(owned,op,host,dict(native_asset=op['asset']),None,None,None),0
        finally:owned.Dispose()
    recipe=source(op);subd=make_source(recipe,host);owned=None;key=None
    try:
        key=doc.Objects.AddSubD(subd)
        if key==S.Guid.Empty:raise ValueError('cannot insert owned soft-crease source')
        obj=doc.Objects.FindId(key);vertices=[];v=obj.Geometry.Vertices.First
        while v is not None:vertices.append(v);v=v.Next
        indices={int(v.Id):i for i,v in enumerate(vertices)}
        wanted=op.get('edge',[7,12] if op['scene']=='grid' else [0,1])
        selected=0
        for edge in obj.Geometry.Edges:
            pair=sorted([indices[int(edge.VertexFrom.Id)],indices[int(edge.VertexTo.Id)]])
            if pair==wanted:
                if not obj.SelectSubObject(edge.ComponentIndex(),True,True,False):raise ValueError('cannot select owned sharp edge')
                selected+=1
        if selected!=1:raise ValueError('ambiguous owned soft-crease edge')
        marker='Owned soft NURBS '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_-SubDCrease '+repr(op['weight'])+' _Enter'
        success,_,events=observe_command(R.Commands.Command,'SubDCrease',lambda:R.RhinoApp.RunScript(macro,True),lambda:None,lambda:[],True)
        if not success or R.Commands.Command.InCommand():raise ValueError('native soft-crease creation failed')
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        owned=doc.Objects.FindId(key).Geometry.Duplicate();doc.Objects.Delete(key,True);key=None
        return convert(owned,op,host,recipe,wanted,history,[e['result'] for e in events if e['name']=='SubDCrease']),0
    finally:
        if key is not None:doc.Objects.Delete(key,True)
        if owned is not None:owned.Dispose()
        subd.Dispose()

def convert(owned,op,host,recipe,wanted,history,soft_results):
    R,S=host['Rhino'],host['System'];G=R.Geometry
    pack_count=int(owned.PackFaces());before=subd_value(owned,host);crc=int(owned.DataCRC(0))
    model=R.FileIO.File3dm()
    try:model.Objects.AddSubD(owned);archive=[int(v) for v in model.ToByteArray()]
    finally:model.Dispose()
    outputs=[]
    for packed in (False,True):
        options=G.SubDToBrepOptions(packed,S.Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,op['mode']));brep=None
        try:
            brep=owned.ToBrep(options)
            if brep is None or not brep.IsValid:raise ValueError('native soft SubD conversion failed')
            outputs.append(dict(valid=True,solid=bool(brep.IsSolid),pack_ids=[int(f.PackId) for f in brep.Faces],geometry=host['_interchange_brep_record'](brep)))
        finally:
            if brep is not None:brep.Dispose()
            options.Dispose()
    if int(owned.DataCRC(0))!=crc:raise ValueError('native soft conversion changed source')
    return dict(input=recipe,soft_edge=wanted,weight=op.get('weight'),before=before,archive=archive,
        pack_count=pack_count,source_unchanged=True,unpacked=outputs[0],output=outputs[1],
        soft_history=history,soft_results=soft_results)
