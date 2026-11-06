"""Fixed getter-admission boundaries and close/Enter controls."""
Z=2.3283064365386963e-10
PRECISION={
 'zero_below':dict(actions=[[0,0,0],[Z/2,0,0],[1,0,0],'Enter']),
 'zero_at':dict(actions=[[0,0,0],[Z,0,0],[1,0,0],'Enter']),
 'zero_above':dict(actions=[[0,0,0],[Z*2,0,0],[1,0,0],'Enter']),
 'zero_diagonal':dict(actions=[[0,0,0],[Z*.75,Z*.75,0],[1,0,0],'Enter']),
 'translated_ulp':dict(actions=[[1e12,1e12,1e12],[1e12+.0001220703125,1e12,1e12],[1e12+4,1e12,1e12],'Enter']),
 'translated_small':dict(actions=[[1e12,1e12,1e12],[1e12+.001,1e12,1e12],[1e12+4,1e12,1e12],'Enter']),
 'close_two_cancel':dict(actions=[[0,0,0],[4,3,2],'Close','Cancel']),
 'close_anchor_cancel':dict(actions=[[0,0,0],'Close','Cancel']),
 'explicit_return':dict(actions=[[0,0,0],[4,0,0],[4,3,1],[0,0,0],'Enter']),
 'enter_anchor':dict(actions=[[0,0,0],'Enter','Cancel']),
 'enter_empty':dict(actions=['Enter','Cancel']),
}
