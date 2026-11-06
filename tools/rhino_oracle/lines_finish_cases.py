"""Corrected bounded completion inputs and independent coincidence checks."""
E=1.4901161193847656e-8
FINISH={
 'explicit_return_finish':dict(actions=[[0,0,0],[4,0,0],[4,3,1],[0,0,0]]),
 'enter_anchor_finish':dict(actions=[[0,0,0],'Enter']),
 'enter_empty_finish':dict(actions=['Enter']),
 'epsilon_below':dict(actions=[[0,0,0],[E*(1-2**-52),0,0],[1,0,0],'Enter']),
 'epsilon_at':dict(actions=[[0,0,0],[E,0,0],[1,0,0],'Enter']),
 'epsilon_above':dict(actions=[[0,0,0],[E*(1+2**-52),0,0],[1,0,0],'Enter']),
 'epsilon_diagonal':dict(actions=[[0,0,0],[E*.75,E*.75,0],[1,0,0],'Enter']),
 'nanometre':dict(actions=[[0,0,0],[1e-9,0,0],[1,0,0],'Enter']),
 'coarse_tolerance':dict(actions=[[0,0,0],[1e-7,0,0],[2e-7,1e-7,0],'Enter'],absolute_tolerance=.01),
 'coarse_epsilon':dict(actions=[[0,0,0],[E*2,0,0],[1,0,0],'Enter'],absolute_tolerance=.01),
 'fine_epsilon':dict(actions=[[0,0,0],[E/2,0,0],[1,0,0],'Enter'],absolute_tolerance=1e-12),
}
