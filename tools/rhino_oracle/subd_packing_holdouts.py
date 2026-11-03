"""Packing holdouts for seams, subdivision, irregular order and coincident shells."""
import math
if __package__:
    from .subd_packing_cases import source as base_source
else:
    from subd_packing_cases import source as base_source

SCENES=('grid_L_shuffled','subdivided_box2','torus_5x3','torus_shifted',
        'torus_first_rotated','torus_crease_loop','cylinder_shifted',
        'cylinder_crease','twisted_torus','coincident_grids')

def cases():
    return [dict(id=scene+'_'+str(mode),op='subd_packing',scene=scene,mode=mode)
            for scene in SCENES for mode in ((0,3,4) if scene=='subdivided_box2' else (0,3))]

def source(scene):
    if scene=='subdivided_box2':return base_source('box')
    if scene=='grid_L_shuffled':
        recipe=base_source('grid_L');recipe['faces']=recipe['faces'][::-1];return recipe
    if scene=='coincident_grids':
        recipe=base_source('two_grids');n=len(recipe['points'])//2
        recipe['points'][n:]=[p[:] for p in recipe['points'][:n]];return recipe
    recipe=base_source('cylinder' if scene.startswith('cylinder') else 'torus')
    if scene.endswith('shifted'):recipe['faces']=recipe['faces'][13:]+recipe['faces'][:13]
    if scene=='torus_first_rotated':recipe['faces']=[f[1:]+f[:1] for f in recipe['faces']]
    if scene=='torus_crease_loop':recipe['creases']=[sorted([x,(x+1)%8]) for x in range(8)]
    if scene=='cylinder_crease':recipe['creases']=[[0,8],[8,16],[16,24]]
    if scene=='torus_5x3':
        w,h=5,3;points=[];faces=[]
        for y in range(h):
            v=2*math.pi*y/h
            for x in range(w):
                u=2*math.pi*x/w;points.append([(4+math.cos(v))*math.cos(u),(4+math.cos(v))*math.sin(u),math.sin(v)])
        for y in range(h):
            for x in range(w):faces.append([y*w+x,y*w+(x+1)%w,((y+1)%h)*w+(x+1)%w,((y+1)%h)*w+x])
        recipe=dict(points=points,faces=faces)
    if scene=='twisted_torus':
        for x in range(8):recipe['faces'][24+x]=[24+x,24+(x+1)%8,(x+2)%8,(x+1)%8]
    return recipe
