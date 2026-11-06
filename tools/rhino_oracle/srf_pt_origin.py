"""Bare zero uses the receiving construction-plane origin at point prompts."""
ORIGINS={
 'zero_quad':dict(actions=[0,[4,0,0],[4,5,0],[0,5,0]]),
 'zero_triangle':dict(actions=[0,[4,0,0],[1,5,0],'Enter']),
 'zero_late_quad':dict(actions=[[1,2,3],[4,-1,5],0,[-2,5,4]]),
 'zero_cplane_triangle':dict(actions=[0,[7,-1,5],[9,6,8],'Enter'],construction_plane=[[2,-1,4],[0.8,0.6,0],[-0.36,0.48,0.8]]),
 'zero_cplane_late':dict(actions=[[1,2,3],[7,-1,5],0,[-2,5,4]],construction_plane=[[2,-1,4],[0.8,0.6,0],[-0.36,0.48,0.8]]),
}
