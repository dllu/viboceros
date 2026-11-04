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
    if (not isinstance(op,dict) or op not in cases()+holdout_cases()+even_cases() or type(op.get('mode')) is not int
            or ('weight' in op and type(op['weight']) is not float)):
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
