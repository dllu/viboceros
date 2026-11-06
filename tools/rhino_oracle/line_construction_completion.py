"""Complete tiny-line diagnostics and independent symmetric/plane controls."""
COMPLETION={
 'tiny_complete_1e12':dict(actions=[[0,0,0],[1e-12,0,0]]),
 'tiny_complete_1e10':dict(actions=[[0,0,0],[1e-10,0,0]]),
 'tiny_complete_zero_tolerance':dict(actions=[[0,0,0],[2.3283064365386963e-10,0,0]]),
 'tiny_complete_1e9':dict(actions=[[0,0,0],[1e-9,0,0]]),
 'both_four':dict(actions=['BothSides','FourPoint',[0,0,0],[4,3,0],[1,2,3],[7,5,2]]),
 'both_bisector':dict(actions=['BothSides','Bisector',[1,2,3],[4,2,3],[1,6,3],[8,5,3]]),
 'both_angled':dict(actions=['BothSides','Angled',[1,2,3],[5,2,3],30,[8,6,3]]),
 'angled_plane':dict(actions=['Angled',[1,2,3],[1,6,3],30,[8,8,7]],construction_plane=[[10,20,30],[0,1,0],[0,0,1]]),
 'vertical_reverse':dict(actions=['Vertical',[1,2,3],5,[8,9,-10]]),
}
