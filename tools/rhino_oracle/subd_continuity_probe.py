"""Owned continuity-mode controls, including asymmetric and dart sectors."""
if __package__:
    from .subd_g0_probe import source as g0_source,SCENES,HOLDOUT_SCENES
    from .subd_to_nurbs_probe import make_source
    from .subd_box_probe import subd_value
else:
    from subd_g0_probe import source as g0_source,SCENES,HOLDOUT_SCENES
    from subd_to_nurbs_probe import make_source
    from subd_box_probe import subd_value

CONTINUITY_SCENES=SCENES+HOLDOUT_SCENES+('skew_box','one_crease_box','crease_chain_box','fan_asymmetric')

def cases():
    return [dict(id=scene+'_'+str(mode),op='subd_continuity',scene=scene,mode=mode)
            for scene in CONTINUITY_SCENES for mode in range(5)]

def validate(op):
    if not isinstance(op,dict) or op not in cases() or type(op.get('mode')) is not int:
        raise ValueError('fixed SubD continuity recipe required')

def source(scene):
    if scene in ('skew_box','one_crease_box','crease_chain_box'):
        recipe=g0_source('box')
        if scene=='skew_box':recipe['points'][0]=[-0.4,0.2,-0.1];recipe['points'][6]=[4.5,2.6,2.8]
        else:recipe['creases']=[[0,1]] if scene=='one_crease_box' else [[0,1],[1,2]]
        return recipe
    if scene=='fan_asymmetric':
        recipe=g0_source('five_fan');recipe['points'][1]=[1.2,0.2,0.7];recipe['points'][7][2]+=0.35;return recipe
    return g0_source(scene)

def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];G=R.Geometry;S=host['System']
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    recipe=source(op['scene']);subd=make_source(recipe,host);brep=None;options=None
    try:
        before=subd_value(subd,host);crc=int(subd.DataCRC(0))
        options=G.SubDToBrepOptions(False,S.Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,op['mode']))
        brep=subd.ToBrep(options)
        if brep is None or not brep.IsValid:raise ValueError('native continuity conversion failed')
        if int(subd.DataCRC(0))!=crc:raise ValueError('native continuity conversion edited source')
        return dict(input=recipe,before=before,source_unchanged=True,
            output=dict(valid=True,solid=bool(brep.IsSolid),geometry=host['_interchange_brep_record'](brep))),0
    finally:
        if options is not None:options.Dispose()
        if brep is not None:brep.Dispose()
        subd.Dispose()
