"""Independent spatial, translated and degenerate corner recipes."""
HOLDOUTS={
 'quad_holdout_a':dict(actions=[[-3,7,2],[4,1,-3],[12,8,5],[1,10,-1]]),
 'quad_holdout_b':dict(actions=[[6,-4,8],[-2,-5,3],[-5,2,-1],[8,6,4]]),
 'quad_small':dict(actions=[[0,0,0],[0.0001,0,0],[0.00013,0.00008,0.00002],[0,0.00005,0]]),
 'quad_large':dict(actions=[[1000000,2000000,-3000000],[1000008,2000001,-2999999],[1000002,2000005,-2999997],[999998,2000004,-3000002]]),
 'triangle_holdout_a':dict(actions=[[-3,7,2],[4,1,-3],[12,8,5],'Enter']),
 'triangle_holdout_b':dict(actions=[[6,-4,8],[-2,-5,3],[-5,2,-1],'Enter']),
 'triangle_small':dict(actions=[[0,0,0],[0.0001,0,0],[0.00013,0.00008,0.00002],'Enter']),
 'repeat_first_second_triangle':dict(actions=[[0,0,0],[0,0,0],[0,5,0],'Enter']),
 'repeat_first_second_quad':dict(actions=[[0,0,0],[0,0,0],[4,5,1],[0,5,0]]),
 'repeat_first_third_quad':dict(actions=[[0,0,0],[4,0,0],[0,0,0],[0,5,0]]),
 'all_coincident_triangle':dict(actions=[[1,2,3],[1,2,3],[1,2,3],'Enter']),
 'all_coincident_quad':dict(actions=[[1,2,3],[1,2,3],[1,2,3],[1,2,3]]),
 'thin_triangle':dict(actions=[[0,0,0],[0.00000001,0,0],[2,5,1],'Enter']),
 'thin_quad':dict(actions=[[0,0,0],[0.00000001,0,0],[0.00000001,5,1],[0,5,1]]),
}
