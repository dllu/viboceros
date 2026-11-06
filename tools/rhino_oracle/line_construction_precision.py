"""Independent tiny-line limits and corrected native completion controls."""
PRECISION={
 'epsilon_below_complete':dict(actions=[[0,0,0],[1.4901161193849996e-8,0,0]]),
 'opposite_complete':dict(actions=['Bisector',[0,0,0],[4,0,0],[-3,0,0]]),
 'tiny_1e12':dict(actions=[[0,0,0],[1e-12,0,0],'Cancel']),
 'tiny_1e10':dict(actions=[[0,0,0],[1e-10,0,0],'Cancel']),
 'tiny_zero_tolerance':dict(actions=[[0,0,0],[2.3283064365386963e-10,0,0],'Cancel']),
 'tiny_1e9':dict(actions=[[0,0,0],[1e-9,0,0],'Cancel']),
 'tiny_repeat_then_end':dict(actions=[[0,0,0],[1e-12,0,0],[4,0,0]]),
 'angled_enter_angle':dict(actions=['Angled',[1,2,3],[5,2,3],'Enter']),
 'macro_vertical':dict(macro='_Line _Vertical w1,2,3 5 w8,9,10\n_Point w7,8,9\n'),
 'macro_cancel':dict(macro='_Line _BothSides w1,2,3 _Enter\n_Point w7,8,9\n'),
}
