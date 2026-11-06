"""Admission around OpenNURBS' published decimal ON_SQRT_EPSILON."""
E=1.490116119385e-8
BOUNDARIES={
 'native_epsilon_below':dict(actions=[[0,0,0],[1.4901161193849996e-8,0,0],[1,0,0],'Enter']),
 'native_epsilon_at':dict(actions=[[0,0,0],[E,0,0],[1,0,0],'Enter']),
 'native_epsilon_above':dict(actions=[[0,0,0],[1.4901161193850002e-8,0,0],[1,0,0],'Enter']),
 'double_back':dict(actions=[[0,0,0],[4,3,2],[0,0,0],'Enter']),
}
