"""Separate corner coincidence from triangular line-edge admission."""
ADMISSION={
 'triangle_edge_1e10':dict(actions=[[0,0,0],[1e-10,0,0],[2,5,1],'Enter']),
 'triangle_edge_zero_tolerance':dict(actions=[[0,0,0],[2.3283064365386963e-10,0,0],[2,5,1],'Enter']),
 'triangle_edge_above_zero_tolerance':dict(actions=[[0,0,0],[2.328306436538697e-10,0,0],[2,5,1],'Enter']),
 'triangle_edge_3e10':dict(actions=[[0,0,0],[3e-10,0,0],[2,5,1],'Enter']),
 'triangle_diagonal_small':dict(actions=[[0,0,0],[2e-10,2e-10,0],[2,5,1],'Enter']),
 'triangle_near_second':dict(actions=[[0,0,0],[4,0,0],[4,1e-10,0],'Enter']),
 'quad_two_adjacent_b':dict(actions=[[0,0,0],[4,0,0],[4,0,0],[4,0,0]]),
 'quad_two_adjacent_a':dict(actions=[[0,0,0],[4,0,0],[0,0,0],[0,0,0]]),
 'quad_one_opposite_b':dict(actions=[[0,0,0],[0,0,0],[4,0,0],[0,0,0]]),
 'quad_two_each':dict(actions=[[0,0,0],[0,0,0],[4,0,0],[4,0,0]]),
}
