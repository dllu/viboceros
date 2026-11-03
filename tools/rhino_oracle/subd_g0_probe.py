"""Owned base-patch controls before extraordinary-vertex postprocessing."""
import math
if __package__:
    from .subd_to_nurbs_probe import source as known_source,make_source
    from .subd_box_probe import subd_value
else:
    from subd_to_nurbs_probe import source as known_source,make_source
    from subd_box_probe import subd_value

SCENES=('quad_corners','quad_smooth','grid','box','pentagon','creased_box','torus',
        'smooth_pentagon','five_fan','seven_fan')
HOLDOUT_SCENES=('pentagonal_prism','tilted_grid','reversed_fan')

def cases(holdouts=False):
    return [dict(id=scene,op='subd_to_nurbs_g0',scene=scene) for scene in (HOLDOUT_SCENES if holdouts else SCENES)]

def validate(op):
    if not isinstance(op,dict) or op not in cases()+cases(True):raise ValueError('fixed G0 patch recipe required')

def source(scene):
    if scene=='pentagonal_prism':
        points=[[3*math.cos(2*math.pi*i/5),3*math.sin(2*math.pi*i/5),2.*z] for z in range(2) for i in range(5)]
        faces=[[i,(i+1)%5,(i+1)%5+5,i+5] for i in range(5)]
        return dict(points=points,faces=faces+[[4,3,2,1,0],[5,6,7,8,9]])
    if scene=='tilted_grid':
        result=known_source('grid');result['points']=[[2*y+5,-x+3,1.3*z+0.4*x] for x,y,z in result['points']];return result
    if scene=='reversed_fan':
        result=source('five_fan');result['faces']=[f[:1]+list(reversed(f[1:])) for f in result['faces']];return result
    if scene=='smooth_pentagon':
        result=known_source('pentagon');result['corners']=[];return result
    if scene in ('five_fan','seven_fan'):
        n=5 if scene=='five_fan' else 7
        points=[[0.,0.,0.4]]
        for i in range(n):
            a=2*math.pi*i/n;points.append([1.5*math.cos(a),1.5*math.sin(a),0.1*math.sin(2*a)])
        for i in range(n):
            a=2*math.pi*(i+0.5)/n;points.append([3*math.cos(a),3*math.sin(a),0.3*math.cos(3*a)])
        return dict(points=points,faces=[[0,1+i,1+n+i,1+(i+1)%n] for i in range(n)],corners=list(range(n+1,2*n+1)))
    return known_source(scene)

def run(op,host,iterations,tolerance):
    validate(op);R=host['Rhino'];G=R.Geometry;S=host['System']
    if iterations!=1 or list(R.RhinoDoc.ActiveDoc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty private idle document required')
    recipe=source(op['scene']);subd=make_source(recipe,host);brep=None;options=None
    try:
        before=subd_value(subd,host);crc=int(subd.DataCRC(0))
        options=G.SubDToBrepOptions(False,S.Enum.ToObject(G.SubDToBrepOptions.ExtraordinaryVertexProcessOption,0))
        brep=subd.ToBrep(options)
        if brep is None or not brep.IsValid:raise ValueError('native G0 patch conversion failed')
        if int(subd.DataCRC(0))!=crc:raise ValueError('native G0 conversion edited source')
        return dict(input=recipe,before=before,source_unchanged=True,
            output=dict(valid=True,solid=bool(brep.IsSolid),geometry=host['_interchange_brep_record'](brep))),0
    finally:
        if options is not None:options.Dispose()
        if brep is not None:brep.Dispose()
        subd.Dispose()
