"""Fixed whole, face and naked-edge extrusions on independent SubD cages."""
TARGETS={
 'quad_whole':('quad','whole',[]),'quad_face':('quad','face',[0]),'quad_edge':('quad','edge',[0]),'quad_loop':('quad','edge',[0,1,2,3]),
 'warped_face':('warped','face',[0]),'bent_whole':('bent','whole',[]),'bent_face':('bent','face',[0]),'bent_faces':('bent','face',[0,1]),'bent_edge':('bent','edge',[0]),
 'grid_face':('grid','face',[1]),'grid_faces':('grid','face',[0,1]),'grid_disjoint':('grid','face',[0,5]),
 'creased_face':('creased','face',[0]),'closed_whole':('closed','whole',[]),'closed_face':('closed','face',[0]),'closed_faces':('closed','face',[0,2]),
 'triangles_face':('triangles','face',[0]),'triangles_faces':('triangles','face',[0,1]),'disjoint_whole':('disjoint','whole',[]),
}


EXTRA_TARGETS={
 'creased_whole':('creased','whole',[]),'creased_faces':('creased','face',[0,1]),'creased_other':('creased','face',[1]),
 'warped_edge':('warped','edge',[0]),'bent_edges':('bent','edge',[0,1]),
 'grid_edges':('grid','edge',[0,1]),
}


def cases(extra=False):
    return [dict(op='subd_extrude_command',id='-'.join((target,mode,str(distance))),target=target,mode=mode,distance=distance)
            for target,(_,kind,_) in (EXTRA_TARGETS if extra else TARGETS).items() for mode in (('Z','N','V')if kind=='edge' else ('Z','N')) for distance in (1.5,-1.5)]


MIXED_TARGETS={
 'bent_redundant':('bent','mixed',dict(faces=[0],edges=[0,1])),
 'bent_separate':('bent','mixed',dict(faces=[0],edges=[6])),
 'creased_redundant':('creased','mixed',dict(faces=[0],edges=[0,1])),
 'grid_mixed':('grid','mixed',dict(faces=[1],edges=[0,1])),
}
def mixed_cases():
    return [dict(op='subd_extrude_command',id='-'.join((target,mode,str(distance))),target=target,mode=mode,distance=distance)
            for target in MIXED_TARGETS for mode in ('Z','N') for distance in (1.5,-1.5)]

def validate(op):
    if (not isinstance(op,dict) or type(op.get('distance'))not in(float,int) or not any(op==row for row in cases()+cases(True)+mixed_cases())):
        raise ValueError('fixed owned SubD extrusion recipe required')
