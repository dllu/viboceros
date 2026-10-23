"""Independent threshold, split-method and channel inputs for quad triangulation."""
from copy import deepcopy
from math import pi
WARP=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,.4]]
FLAT=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.]]
SADDLE=[[-1.,-1.,.125],[1.,-1.,-.125],[1.,1.,.125],[-1.,1.,-.125]]
METHODS=['ShortestDiagonal','LongestDiagonal','MinimizeArea','MaximizeArea','MinimumAngle','MaximumAngle']
CASES={}


def add(name,vertices=WARP,faces=None,distance=.01,angle=None,method=1,**extra):
    CASES['nonplanar_'+name]=dict(vertices=deepcopy(vertices),faces=deepcopy(faces or [[0,1,2,3]]),
        distance=distance,angle=angle,method=method,**extra)


for i,name in enumerate(METHODS,1):
    add('distance_'+name,method=i)
    add('angle_'+name,distance=None,angle=.25,method=i)
add('flat_distance_zero',FLAT,distance=0.)
add('flat_angle_zero',FLAT,distance=None,angle=0.)
add('flat_both_zero',FLAT,distance=0.,angle=0.)
for name,distance in [('equal',.25),('below',.24999999999999997),('above',.25000000000000006)]:
    add('distance_'+name,SADDLE,distance=distance)
add('both_distance_only',distance=.01,angle=pi)
add('both_angle_only',distance=10.,angle=.01)
add('both_neither',distance=10.,angle=pi)
add('channels_ngon',colors=[[255,0,0,0],[0,255,0,127],[0,0,255,255],[90,80,70,0]],
    uv=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],normals=[[1.,0.,0.]]*4,
    ngons=[dict(vertices=[0,1,2,3],faces=[0])])
add('multiple_faces',[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,1.,.4],[2.,1.,0.],[4.,1.,0.]],
    faces=[[0,1,4,3],[1,2,5,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1])])
for method in [1,2,3,5]:
    add('concave_'+str(method),[[0.,0.,0.],[3.,0.,0.],[1.,.5,.2],[0.,2.,0.]],method=method)
add('tiny',[[x*1e-8 for x in p]for p in WARP],distance=1e-10)
add('translated',[[x+1e6,y-2e6,z+3e6]for x,y,z in WARP])
add('unused_vertex',WARP+[[99.,99.,99.]])
add('reversed',faces=[[0,3,2,1]])
add('cyclic',faces=[[1,2,3,0]],method=3)


MAIN = dict(CASES)
NGONS = {}
for name,order,all_warp in [
    ('reverse_owners',[2,1,0],False), ('preserved_first',[2,0,1],False),
    ('preserved_middle',[0,2,1],False), ('all_removed',[2,1,0],True),
]:
    vertices=[]
    for i,shape in enumerate([WARP,WARP,WARP if all_warp else FLAT]):
        vertices.extend([[x+4*i,y,z]for x,y,z in shape])
    rings=[list(range(4*i,4*i+4))for i in range(3)]
    add('ngons_'+name,vertices,faces=rings,ngons=[dict(vertices=rings[i],faces=[i])for i in order])
    key='nonplanar_ngons_'+name;NGONS[key]=CASES[key]


ANGLES = {}
for i,name in enumerate(METHODS,1):
    # The two diagonal-normal angles straddle .423 radians. The native
    # qualification tests both corner pairs before choosing a diagonal.
    key='nonplanar_angle_discriminator_'+name
    add('angle_discriminator_'+name,distance=None,angle=.423,method=i)
    ANGLES[key]=CASES[key]


def request(ngons=False,angles=False):
    if ngons and angles:raise ValueError('choose one nonplanar reference family')
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
        operations=[dict(op='nonplanar_quad_conversion',id=k,case=k)for k in (ANGLES if angles else NGONS if ngons else MAIN)])


def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='nonplanar_quad_conversion'and op['case']in CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed nonplanar quad recipe required')
