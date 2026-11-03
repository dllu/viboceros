"""Owned face-inset control nets, including groups, holes and concave regions."""
if __package__:
    from .subd_offset_cases import scenes as base_scenes
else:
    from subd_offset_cases import scenes as base_scenes


def scenes():
    nets=base_scenes()
    nets['pentagon']=dict(direct=True,points=[[0.,0.,0.],[4.,0.,0.],[5.,2.,0.],[2.,4.,0.],[-1.,2.,0.]],faces=[[0,1,2,3,4]])
    nets['concave']=dict(direct=True,points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[2.,1.,0.],[0.,3.,0.]],faces=[[0,1,2,3,4]])
    points=[[float(2*x),float(2*y),0.]for y in range(4)for x in range(4)]
    faces=[[4*y+x,4*y+x+1,4*(y+1)+x+1,4*(y+1)+x]for y in range(3)for x in range(3)if(x,y)!=(1,1)]
    nets['ring']=dict(points=points,faces=faces)
    nets['collinear']=dict(points=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,3.,0.]],faces=[[0,1,2,3]])
    nets['skew']=dict(points=[[0.,0.,0.],[4.,0.,0.],[3.,3.,0.],[-1.,2.,0.]],faces=[[0,1,2,3]])
    nets['large']=dict(points=[[1e7+x,2e7+y,-3e7+z]for x,y,z in nets['warped']['points']],faces=[[0,1,2,3]])
    return nets

TARGETS={
 'quad':('quad',[0]),'warped':('warped',[0]),'bent_face':('bent',[0]),'bent_both':('bent',[0,1]),
 'creased_face':('creased',[0]),'creased_both':('creased',[0,1]),
 'grid_face':('grid',[1]),'grid_pair':('grid',[0,1]),'grid_concave':('grid',[0,1,3]),'grid_vertex':('grid',[0,4]),
 'closed_face':('closed',[0]),'closed_both':('closed',[0,2]),'closed_all':('closed',list(range(6))),
 'triangles_face':('triangles',[0]),'triangles_both':('triangles',[0,1]),
 'disjoint':('disjoint',[0,1]),'pentagon':('pentagon',[0]),'concave':('concave',[0]),'ring':('ring',list(range(8))),
}

def cases():
    result=[]
    for target,(_,faces)in TARGETS.items():
        for mode in (('Group','Single')if len(faces)>1 else ('Group',)):
            for distance in (.3,-.3):
                result.append(dict(op='inset_faces',id='-'.join((target,mode,str(distance))),target=target,mode=mode,distance=distance))
    for distance in (0.,2.,10.):result.append(dict(op='inset_faces',id='quad-Group-'+str(distance),target='quad',mode='Group',distance=distance))
    return result


def validate(op):
    if not isinstance(op,dict) or type(op.get('distance'))not in(float,int)or not any(op==row for row in cases()):
        raise ValueError('fixed owned inset recipe required')


def mesh_cases():
    targets=['quad','warped','bent_face','bent_both','grid_face','grid_pair','grid_concave','closed_face','closed_all','triangles_face','triangles_both','disjoint','pentagon','concave','ring','quad_channels']
    return [dict(op='inset_mesh_faces',id='-'.join((target,str(distance))),target=target,mode='Single',distance=distance)for target in targets for distance in (.3,-.3)]

def validate_mesh(op):
    if not isinstance(op,dict)or type(op.get('distance'))not in(float,int)or not any(op==row for row in mesh_cases()):
        raise ValueError('fixed owned mesh inset recipe required')


def mesh_confirmed_cases():
    return [dict(op='inset_mesh_confirmed',id=row['id'],target=row['target'],mode=row['mode'],distance=row['distance'])for row in mesh_cases()if row['distance']>0.]

def validate_mesh_confirmed(op):
    if not isinstance(op,dict)or type(op.get('distance'))not in(float,int)or not any(op==row for row in mesh_confirmed_cases()):
        raise ValueError('fixed owned confirmed mesh inset recipe required')


def mesh_primed_cases():
    return [dict(row,op='inset_mesh_primed',id=row['target']+'-'+str(distance),distance=distance)for row in mesh_cases()if row['distance']>0. for distance in (.3,.7)]

def validate_mesh_primed(op):
    if not isinstance(op,dict)or type(op.get('distance'))not in(float,int)or not any(op==row for row in mesh_primed_cases()):
        raise ValueError('fixed owned primed mesh inset recipe required')


def mesh_ready_cases():
    return [dict(row,op='inset_mesh_ready')for row in mesh_primed_cases()]

def validate_mesh_ready(op):
    if not isinstance(op,dict)or type(op.get('distance'))not in(float,int)or not any(op==row for row in mesh_ready_cases()):
        raise ValueError('fixed owned ready mesh inset recipe required')


EXTRA_TARGETS={'quad_whole':('quad',None),'bent_whole':('bent',None),'closed_whole':('closed',None),'grid_all':('grid',list(range(6))),'collinear':('collinear',[0]),'skew':('skew',[0]),'large':('large',[0])}
def extra_cases():
    return [dict(op='inset_extra_faces',id=target+'-'+mode,target=target,mode=mode,distance=.3)for target,(_,faces)in EXTRA_TARGETS.items()for mode in (('Group','Single')if faces is None and target!='quad_whole'or faces is not None and len(faces)>1 else ('Group',))]

def validate_extra(op):
    if not isinstance(op,dict)or type(op.get('distance'))not in(float,int)or not any(op==row for row in extra_cases()):raise ValueError('fixed owned extra inset recipe required')
