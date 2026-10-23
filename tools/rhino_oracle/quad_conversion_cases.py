"""Fixed independent input meshes for public triangle-to-quad conversion."""
from copy import deepcopy

SQUARE = [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.]]
TRIS = [[0,1,2],[0,2,3]]
SHORT = [[0.,0.,0.],[3.,0.,0.],[1.,1.,0.],[0.,1.,0.]]
EXTRA = [[10.,0.,0.],[11.,0.,0.],[10.,1.,0.]]
def mesh(vertices, faces, angle=.02, ratio=.5, **extra):
    return dict(vertices=deepcopy(vertices),faces=deepcopy(faces),angle=angle,ratio=ratio,**extra)
CASES = {
 'quad_convert_square': mesh(SQUARE,TRIS),
 'quad_convert_square_extra': mesh(SQUARE+EXTRA,TRIS+[[4,5,6]]),
 'quad_convert_short_two': mesh(SHORT,TRIS,ratio=0.),
 'quad_convert_short_extra': mesh(SHORT+EXTRA,TRIS+[[4,5,6]],ratio=0.),
 'quad_convert_tied_fan': mesh([[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],[0.,1.5,0.]],[[0,1,4],[1,2,4],[2,3,4]],ratio=0.),
 'quad_convert_unequal_fan': mesh([[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],[0.,.75,0.]],[[0,1,4],[1,2,4],[2,3,4]],ratio=0.),
 'quad_convert_raw_seam': mesh(SQUARE[:3]+[SQUARE[0],SQUARE[2],SQUARE[3]],[[0,1,2],[3,4,5]]),
 'quad_convert_opposed': mesh(SQUARE,[[0,1,2],[0,3,2]]),
 'quad_convert_channels_ngon': mesh(SQUARE,TRIS,colors=[[255,0,0,255],[0,255,0,128],[0,0,255,0],[90,80,70,255]],uv=[[0.,0.],[1.,0.],[1.,1.],[0.,1.]],normals=[[0.,0.,1.]]*4,ngons=[dict(vertices=[0,1,2,3],faces=[0,1])]),
 'quad_convert_adjacent_ngons': mesh(SQUARE,TRIS,ngons=[dict(vertices=[0,1,2],faces=[0]),dict(vertices=[0,2,3],faces=[1])]),
 'quad_convert_warp_reject': mesh([[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,1.]],TRIS),
 'quad_convert_warp_accept': mesh([[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,1.]],TRIS,angle=1.5707963267948966),
 'quad_convert_angle_reset': mesh([[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,1.]],TRIS,angle=3.141592653589793),
 'quad_convert_unused_vertex': mesh(SQUARE+[[100.,100.,100.]],TRIS),
}

def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='mesh_quad_conversion',id=k,case=k)for k in CASES])
def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='mesh_quad_conversion'and op['case']in CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed quad conversion recipe required')
