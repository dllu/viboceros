"""Owned rectangular, wrapped and irregular control nets for face packing."""
import math
if __package__:
    from .subd_g0_probe import source as known_source
    from .subd_g1x_probe import source as fan_source
else:
    from subd_g0_probe import source as known_source
    from subd_g1x_probe import source as fan_source

SCENES=('grid_2x1','grid_1x4','grid_2x2','grid','grid_smooth','grid_rotated',
        'grid_reversed','grid_shuffled','grid_crease','grid_short_crease',
        'grid_L','grid_hole','grid_T','cylinder','cylinder_rotated','torus',
        'torus_rotated','torus_crease','two_grids','box','subdivided_box',
        'pentagonal_prism','five_fan','seven_fan','fan6','fan8')

def cases():
    return [dict(id=scene+'_'+str(mode),op='subd_packing',scene=scene,mode=mode)
            for scene in SCENES for mode in ((0,3,4) if scene in ('fan6','fan8','subdivided_box') else (0,3))]

def mode_cases():
    return [dict(id=scene+'_'+str(mode),op='subd_packing',scene=scene,mode=mode)
            for scene in ('subdivided_box','subdivided_box2') for mode in (1,2)]

def grid(w,h,active=None):
    cells=active if active is not None else [(x,y) for y in range(h) for x in range(w)]
    old_faces=[[y*(w+1)+x,y*(w+1)+x+1,(y+1)*(w+1)+x+1,(y+1)*(w+1)+x] for x,y in cells]
    used=sorted({v for f in old_faces for v in f});remap={v:i for i,v in enumerate(used)}
    points=[]
    for i in used:
        y,x=divmod(i,w+1);points.append([float(x),float(y),0.15*x*y+0.07*x*x])
    faces=[[remap[v] for v in face] for face in old_faces]
    neighbors=[set() for _ in points]
    for f in faces:
        for a,b in zip(f,f[1:]+f[:1]):neighbors[a].add(b);neighbors[b].add(a)
    return dict(points=points,faces=faces,corners=[i for i,n in enumerate(neighbors) if len(n)==2])

def source(scene):
    if scene in ('fan6','fan8'):return fan_source(scene)
    if scene in ('box','subdivided_box','pentagonal_prism','five_fan','seven_fan'):
        return known_source('box' if scene=='subdivided_box' else scene)
    if scene.startswith('torus') or scene.startswith('cylinder'):
        closed=scene.startswith('torus');w,h=(8,4) if closed else (8,3)
        points=[];faces=[]
        for y in range(h if closed else h+1):
            v=2*math.pi*y/h
            for x in range(w):
                u=2*math.pi*x/w
                r=4+math.cos(v) if closed else 2+0.1*y
                points.append([r*math.cos(u),r*math.sin(u),math.sin(v) if closed else float(y)])
        for y in range(h):
            for x in range(w):faces.append([y*w+x,y*w+(x+1)%w,((y+1)%(h if closed else h+1))*w+(x+1)%w,((y+1)%(h if closed else h+1))*w+x])
        recipe=dict(points=points,faces=faces)
        if scene=='torus_crease':recipe['creases']=[[x,(x+1)%w] for x in range(w)]
    else:
        dimensions={'grid_2x1':(2,1),'grid_1x4':(1,4),'grid_2x2':(2,2)}
        if scene=='grid_L':recipe=grid(4,3,[(x,y) for y in range(3) for x in range(4) if y==0 or x<2])
        elif scene=='grid_hole':recipe=grid(3,3,[(x,y) for y in range(3) for x in range(3) if (x,y)!=(1,1)])
        elif scene=='grid_T':recipe=grid(5,3,[(x,y) for y in range(3) for x in range(5) if y==0 or x==2])
        else:recipe=grid(*dimensions.get(scene,(4,3)))
        if scene=='grid_smooth':recipe['corners']=[]
        if scene=='grid_crease':recipe['creases']=[[5*y+2,5*(y+1)+2] for y in range(3)]
        if scene=='grid_short_crease':recipe['creases']=[[7,12]]
        if scene=='two_grids':
            n=len(recipe['points']);recipe['points'] += [[x+8,y-1,z+2] for x,y,z in recipe['points'][:]]
            recipe['faces'] += [[i+n for i in f] for f in recipe['faces'][:]]
            recipe['corners'] += [i+n for i in recipe['corners'][:]]
    if scene.endswith('rotated'):
        recipe['faces']=[f[i%4:]+f[:i%4] for i,f in enumerate(recipe['faces'])]
    if scene=='grid_reversed':recipe['faces']=[f[:1]+list(reversed(f[1:])) for f in recipe['faces']]
    if scene=='grid_shuffled':recipe['faces']=recipe['faces'][::2][::-1]+recipe['faces'][1::2]
    return recipe
