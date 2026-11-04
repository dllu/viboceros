"""Fixed soft-crease construction and NURBS conversion controls."""
SCENES=('grid','box','torus','five_fan','pentagonal_prism')
WEIGHTS=(10.,33.333,50.,75.,100.)

def cases():
    rows=[dict(id=scene+'_'+str(weight)+'_0',op='subd_soft_nurbs',scene=scene,weight=weight,mode=0)
        for scene in SCENES for weight in WEIGHTS]
    rows += [dict(id=scene+'_'+str(weight)+'_'+str(mode),op='subd_soft_nurbs',scene=scene,weight=weight,mode=mode)
        for scene in ('box','five_fan') for weight in (50.,100.) for mode in (1,2,3,4)]
    return rows

def validate(op):
    if (not isinstance(op,dict) or op not in cases()+holdout_cases()+even_cases()+chart_cases()+offset_cases() or type(op.get('mode')) is not int
            or ('weight' in op and type(op['weight']) is not float)
            or ('edge' in op and (type(op['edge']) is not list or any(type(v) is not int for v in op['edge'])))):
        raise ValueError('fixed owned soft-SubD NURBS recipe required')

ASSETS=('variable_both_ends','variable_chain','variable_branch','hard_soft_chain')
def even_cases():
    return [dict(id='even_'+scene+'_'+str(weight)+'_'+str(mode),op='subd_soft_nurbs',scene=scene,weight=weight,mode=mode)
        for scene in ('fan6','fan8') for weight in (50.,90.,100.) for mode in range(5)]

def holdout_cases():
    rows=[dict(id='fraction_'+scene+'_'+str(weight)+'_'+str(mode),op='subd_soft_nurbs',scene=scene,weight=weight,mode=mode)
        for scene in ('box','five_fan') for weight in (60.,70.,80.,90.) for mode in (0,3)]
    rows += [dict(id=asset+'_'+str(mode),op='subd_soft_nurbs',asset=asset,mode=mode)
        for asset in ASSETS for mode in range(5)]
    return rows

PROFILES=(('five_shifted','five_fan'),('six_shifted','fan6'),('eight_rotated','fan8'),
          ('six_affine','fan6'),('eight_asymmetric','fan8'))
def chart_cases():
    return [dict(id='charts_'+profile+'_'+str(weight)+'_'+str(mode),op='subd_soft_nurbs',
                 scene=scene,profile=profile,weight=weight,mode=mode)
            for profile,scene in PROFILES for weight in (60.,100.) for mode in range(5)]

def offset_cases():
    return [dict(id='offset_'+scene+'_'+str(weight)+'_'+str(mode),op='subd_soft_nurbs',
                 scene=scene,edge=[0,3],weight=weight,mode=mode)
            for scene in ('five_fan','fan6','fan8') for weight in (60.,100.) for mode in (0,3,4)]

def source(op):
    if __package__:
        from .subd_packing_cases import source as packing_source
    else:
        from subd_packing_cases import source as packing_source
    recipe=packing_source(op['scene']);profile=op.get('profile','')
    if profile.endswith('_shifted'):
        recipe['faces']=recipe['faces'][2:]+recipe['faces'][:2]
    elif profile.endswith('_rotated'):
        recipe['faces']=[f[(i+1)%4:]+f[:(i+1)%4] for i,f in enumerate(recipe['faces'])]
    elif profile.endswith('_affine'):
        recipe['points']=[[1.5*x+0.2*z+1.,0.8*y-3.,2.*z+0.1*x] for x,y,z in recipe['points']]
    elif profile=='eight_asymmetric':
        recipe['points'][1]=[1.2,0.2,0.7];recipe['points'][10][2]+=0.35
    return recipe
