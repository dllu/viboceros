"""Fixed physical mesh edge-chain recipes for the owned 3DFace oracle."""
STRIP_VERTICES=[[x,float(y),0.]for y in range(4)for x in [0.,4.]]
STRIP_FACES=[[2*y,2*y+1,2*y+3,2*y+2]for y in range(3)]
def edge(x,y):return dict(edge_pick=[float(x),float(y),0.])
def strip(points,**options):return dict(append=True,multiple=True,chain=True,source_vertices=STRIP_VERTICES,source_faces=STRIP_FACES,points=points,**options)
DRAFTS={
 'edge_multiple':dict(append=True,multiple=True,points=[edge(4,1.5),[7.,0.,0.],[7.,3.,0.],edge(0,1.5),[-3.,3.,0.],[-3.,0.,0.]]),
 'chain_strip':strip([edge(4,.5),[7.,0.,0.],[7.,1.,0.],[7.,2.,0.],[7.,3.,0.]]),
 'chain_reverse':strip([edge(4,2.5),[7.,3.,0.],[7.,2.,0.],[7.,1.,0.],[7.,0.,0.]]),
 'chain_wrap':dict(append=True,multiple=True,chain=True,points=[edge(4,1.5),[7.,0.,0.],[7.,6.,0.],[0.,6.,0.],[-3.,0.,0.],[0.,-3.,0.]]),
 'chain_single':dict(append=True,chain=True,points=[edge(4,1.5),[7.,0.,0.],[7.,3.,0.]]),
 'chain_fold':strip([edge(4,.5),[7.,0.,3.],[7.,1.,3.],[7.,2.,3.],[7.,3.,3.]]),
 'chain_crease':strip([edge(4,.5),[7.,0.,3.],[7.,1.,3.],[7.,2.,3.],[7.,3.,3.]],weld_angle=30.),
 'chain_undo':strip([edge(4,.5),[7.,0.,0.],[7.,1.,0.],[7.,2.,0.],'Undo',[8.,2.,0.],[8.,3.,0.]]),
 'edge_triangles':dict(append=True,multiple=True,polygon='Triangle',points=[edge(4,1.5),[7.,1.5,0.],edge(0,1.5),[-3.,1.5,0.]]),
 'edge_ngons':dict(append=True,multiple=True,polygon='Ngon',points=[edge(4,1.5),[7.,0.,0.],[8.,1.5,0.],[7.,3.,0.],'Enter',edge(0,1.5),[-3.,3.,0.],[-4.,1.5,0.],[-3.,0.,0.],'Enter']),
 'chain_new_boundary':dict(append=True,multiple=True,chain=True,points=[edge(4,1.5),[7.,0.,0.],[7.,3.,0.],'Enter',edge(7,1.5),[10.,0.,0.],[10.,3.,0.]]),
 'chain_cancel':strip([edge(4,.5),[7.,0.,0.],[7.,1.,0.],[7.,2.,0.],[8.,3.,0.]],cancel=True),
}

def request():
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='face_create_command',id=name,case=name)for name in DRAFTS])

EXTRAS={
 'edge_collinear_outer':dict(append=True,points=[edge(0,1.5),[0.,6.,0.],[-3.,0.,0.]]),
 'edge_collinear_inner':dict(append=True,points=[edge(0,1.5),[0.,-3.,0.],[-3.,3.,0.]]),
 'edge_flip_plain':dict(append=True,points=[edge(4,1.5),[7.,3.,0.],[7.,0.,0.]]),
 'chain_corner_turn':dict(append=True,multiple=True,chain=True,points=[edge(4,1.5),[7.,0.,0.],[7.,6.,0.],[-3.,6.,0.],[-3.,-3.,0.],[7.,-3.,0.]]),
 'chain_wrap_shift':dict(append=True,multiple=True,chain=True,points=[edge(4,1.5),[7.,0.,0.],[7.,6.,0.],[-1.,6.,0.],[-3.,0.,0.],[0.,-3.,0.]]),
 'chain_closed_regular':dict(append=True,multiple=True,chain=True,points=[edge(4,1.5),[7.,0.,0.],[7.,6.,0.],[-3.,6.,0.],[-3.,-3.,0.],[7.,-3.,0.],[10.,0.,0.]]),
}
ALL_DRAFTS=dict(DRAFTS,**EXTRAS)

def extra_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in EXTRAS];return q

# Independent corner probes retain partial chains so the next edge can be inferred.
_PREFIX=[edge(4,1.5),[7.,0.,0.],[7.,6.,0.]]
def corner(points,**options):return dict(append=True,multiple=True,chain=True,points=points,native_validity=True,**options)
CORNERS={
 'chain_collinear_stop':corner(_PREFIX+[[0.,6.,0.],[-3.,0.,0.]]),
 'chain_collinear_follow':corner(_PREFIX+[[0.,6.,0.],[-3.,0.,0.],[0.,-3.,0.],[-6.,-3.,0.]]),
 'chain_collinear_left':corner(_PREFIX+[[0.,6.,0.],[-3.,3.,0.],[0.,-3.,0.]]),
 'chain_collinear_right':corner(_PREFIX+[[0.,6.,0.],[3.,0.,0.],[0.,-3.,0.]]),
 'chain_near_positive':corner(_PREFIX+[[1e-7,6.,0.],[-3.,0.,0.],[0.,-3.,0.]]),
 'chain_near_negative':corner(_PREFIX+[[-1e-7,6.,0.],[-3.,0.,0.],[0.,-3.,0.]]),
}
def transformed(case,transform):
 from copy import deepcopy
 case=deepcopy(case)
 source=case.get('source_vertices',[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]])
 case['source_vertices']=[transform(p)for p in source]
 for point in case['points']:
  if isinstance(point,dict):point['edge_pick']=transform(point['edge_pick'])
 case['points']=[transform(p)if isinstance(p,list)else p for p in case['points']]
 return case
CORNERS['chain_collinear_rotated']=transformed(CORNERS['chain_collinear_follow'],lambda p:[2.-p[1],p[0],1.+p[2]])
CORNERS['chain_collinear_scaled']=transformed(CORNERS['chain_collinear_follow'],lambda p:[.5*v for v in p])
CORNERS['edge_overlap_plain']=dict(append=True,native_validity=True,points=[edge(0,1.5),[0.,-3.,0.],[-3.,3.,0.]])
CORNERS['edge_overlap_reversed']=dict(CORNERS['edge_overlap_plain'],source_faces=[[3,2,1,0]])
CORNERS['edge_overlap_near_positive']=dict(append=True,native_validity=True,points=[edge(0,1.5),[1e-7,-3.,0.],[-3.,3.,0.]])
CORNERS['edge_overlap_near_negative']=dict(append=True,native_validity=True,points=[edge(0,1.5),[-1e-7,-3.,0.],[-3.,3.,0.]])
ALL_DRAFTS.update(CORNERS)

def corner_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in CORNERS];return q

CORNER_CONTROLS={
 'chain_frame_tilted':transformed(CORNERS['chain_collinear_follow'],lambda p:[p[0],p[1],p[0]+p[1]+p[2]]),
 'chain_frame_near_tilted':transformed(CORNERS['chain_near_positive'],lambda p:[p[0],p[1],p[0]+p[1]+p[2]]),
 'edge_overlap_tilted':transformed(CORNERS['edge_overlap_plain'],lambda p:[p[0],p[1],p[0]+p[1]+p[2]]),
 'chain_frame_backface':dict(CORNERS['chain_collinear_follow'],source_faces=[[3,2,1,0]]),
 'chain_frame_undo_one':corner(_PREFIX+[[0.,6.,0.],[-3.,0.,0.],'Undo',[-4.,0.,0.],[0.,-4.,0.],[-6.,-4.,0.]]),
 'chain_frame_undo_two':corner(_PREFIX+[[0.,6.,0.],[-3.,0.,0.],'Undo','Undo',[-4.,0.,0.],[0.,-4.,0.],[-6.,-4.,0.]]),
 'chain_frame_mirror':transformed(CORNERS['chain_collinear_follow'],lambda p:[-p[0],p[1],p[2]]),
 'chain_frame_no_weld':dict(CORNERS['chain_collinear_follow'],weld_angle=0.),
 'chain_frame_right_follow':corner(_PREFIX+[[0.,6.,0.],[3.,0.,0.],[0.,-3.,0.],[-6.,-3.,0.]]),
}
ALL_DRAFTS.update(CORNER_CONTROLS)

def corner_control_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in CORNER_CONTROLS];return q

CORNER_FINISH={
 'face_free_cross':dict(from_edge=False,native_validity=True,points=[[0.,0.,0.],[4.,3.,0.],[0.,3.,0.],[4.,0.,0.]]),
 'face_free_cross_reverse':dict(from_edge=False,native_validity=True,points=[[0.,0.,0.],[-4.,3.,0.],[0.,3.,0.],[-4.,0.,0.]]),
 'face_free_overlap':dict(from_edge=False,native_validity=True,points=[[0.,0.,0.],[0.,3.,0.],[0.,-3.,0.],[-3.,3.,0.]]),
 'edge_undo_seed':dict(append=True,native_validity=True,points=[edge(4,1.5),'Undo',[7.,0.,0.],[7.,3.,0.],[10.,3.,0.]]),
}
ALL_DRAFTS.update(CORNER_FINISH)

def corner_finish_request():
 q=request();q['operations']=[dict(op='face_create_command',id=name,case=name)for name in CORNER_FINISH];return q
