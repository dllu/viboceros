"""Select a polysurface once, then move the base to another face."""
if __package__:from .line_normal_cases import case
else:from line_normal_cases import case
BOX=dict(type='box_brep',min=[0,0,0],max=[10,10,10])
BREP={
 'box_side_after_top':dict(case(BOX,[5,5,10],[0,3,4],[-6,5,7]),shaded=True),
 'box_top_normal':dict(case(BOX,[5,5,10],[4,3,10],[7,8,14]),shaded=True),
 'box_bottom_numeric':dict(case(BOX,[5,5,10],[4,3,0],5),shaded=True),
 'box_ignore_face':dict(curves=[BOX],shaded=True,actions=['Normal','IgnoreTrims=Yes',dict(curve=0,pick=[5,5,10]),[0,3,4],5]),
}
