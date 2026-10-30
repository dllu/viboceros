"""Authored SubD wall cages; no observed output is used as an input."""
from copy import deepcopy

RINGS = {
    'square': [[0.,0.],[4.,0.],[4.,3.],[0.,3.]],
    'triangle': [[0.,0.],[4.,0.],[2.,3.]],
    'pentagon': [[0.,0.],[4.,0.],[5.,2.],[2.,4.],[-1.,2.]],
    'concave': [[0.,0.],[3.,0.],[3.,1.],[1.,1.],[1.,3.],[0.,3.]],
}


def walls(shape,reverse=False):
    rings=[RINGS['square'if shape=='annulus'else shape]]
    if shape=='annulus':rings.append([[1.,1.],[3.,1.],[3.,2.],[1.,2.]])
    vertices=[];faces=[]
    for index,ring in enumerate(rings):
        n=len(ring);base=len(vertices)
        for z in [0.,2.]:vertices.extend([[x,y,z]for x,y in ring])
        for side in range(n):
            nxt=(side+1)%n
            face=[base+side,base+nxt,base+nxt+n,base+side+n]
            if bool(index)^reverse:face.reverse()
            faces.append(face)
    return dict(kind='subd',vertices=vertices,faces=faces)


CASES={}
def add(name,shape='square',crease=True,triangles=False,delete=True,reverse=False):
    CASES[name]=dict(command='Cap',sources=[walls(shape,reverse)],whole=True,preselected=True,
        command_arguments=' '.join('_'+key+'=_'+('Yes'if value else'No')for key,value in
            [('DeleteInput',delete),('Crease',crease),('Triangles',triangles)]))

for crease in [True,False]:
    for triangles in [False,True]:
        add('square-'+('crease'if crease else'smooth')+'-'+('triangles'if triangles else'face'),crease=crease,triangles=triangles)
for shape in ['triangle','pentagon','concave','annulus']:
    for triangles in [False,True]:add(shape+'-'+('triangles'if triangles else'face'),shape,triangles=triangles)
for crease in [True,False]:add('reversed-'+('crease'if crease else'smooth'),crease=crease,reverse=True)
add('nonplanar');CASES['nonplanar']['sources'][0]['vertices'][4][2]+=0.25
add('copy',delete=False)
add('closed-noop');CASES['closed-noop'].pop('command_arguments')
CASES['closed-noop']['sources'][0]['faces'].extend([[0,3,2,1],[4,5,6,7]])
add('sheet');CASES['sheet']['sources']=[dict(kind='subd',vertices=[[x,y,0.]for x,y in RINGS['square']],faces=[[0,1,2,3]])]

BASIC_CASES=tuple(CASES)
HOLDOUT_CASES=[]
for name,ring in [
    ('skew-quad',[[0.,0.],[6.,0.],[4.,3.],[0.,3.]]),
    ('skew-pentagon',[[0.,0.],[6.,0.],[8.,2.],[5.,5.],[0.,3.]]),
    ('diagonal-quad',[[0.,2.],[2.,0.],[2.,1.],[1.,2.]]),
]:
    RINGS[name]=ring;add(name,name,triangles=True);HOLDOUT_CASES.append(name)
add('tilted-skew','skew-quad',triangles=True)
CASES['tilted-skew']['sources'][0]['vertices']=[[16.+x-y+z,-32.+x+y-z,8.+2.*y+z]for x,y,z in CASES['tilted-skew']['sources'][0]['vertices']]
HOLDOUT_CASES.append('tilted-skew')
for crease in [True,False]:
    name='corner-'+('crease'if crease else'smooth');add(name,crease=crease)
    CASES[name]['sources'][0]['corners']=[0];HOLDOUT_CASES.append(name)
    name='spine-'+('crease'if crease else'smooth');add(name,crease=crease)
    CASES[name]['sources'][0]['creases']=[[0,4]];HOLDOUT_CASES.append(name)


def validate(op):
    if not(isinstance(op,dict)and set(op)=={'op','id','case'}and op.get('op')=='subd_cap_regions'
           and isinstance(op.get('case'),str)and op['case']in CASES and op['id']==op['case']):
        raise ValueError('fixed owned SubD cap recipe required')
