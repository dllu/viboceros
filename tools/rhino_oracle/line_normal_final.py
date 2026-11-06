"""Shaded source clicks and independent completion/normal-plane holdouts."""
if __package__:
 from .line_normal_cases import case,CIRCLE,CURVED,PLANE,SPATIAL
 from .line_normal_holdouts import patch,trimmed,LINE
else:
 from line_normal_cases import case,CIRCLE,CURVED,PLANE,SPATIAL
 from line_normal_holdouts import patch,trimmed,LINE
FINAL={
 'trim_shaded_no':dict(trimmed(False),shaded=True),
 'trim_shaded_yes':dict(trimmed(True),shaded=True),
 'trim_shaded_flipped':dict(trimmed(False,True),shaded=True),
 'reversed_shaded_point':dict(case(patch('brep',True),[2,3,0],[4,3,2],[8,9,10]),shaded=True),
 'reversed_shaded_numeric':dict(case(patch('brep',True),[2,3,0],[4,3,2],5),shaded=True),
 'line_numeric_point':case(LINE,[5,0,0],[5,0,0],5,[7,4,3]),
 'line_increment_point':case(LINE,[5,0,0],[5,0,0],-2,[7,4,3]),
 'line_normal_both':case(LINE,[5,0,0],[5,0,0],[7,4,3],both=True),
 'spatial_linear_normal':case(dict(type='line',start=[0,0,0],end=[10,5,3]),[5,2.5,1.5],[5,2.5,1.5],[7,4,8]),
 'surface_normal_both':case(CURVED,[5,5,1],[5,5,1],[8,7,6],both=True),
 'surface_numeric_both':case(CURVED,[5,5,1],[5,5,1],5,both=True),
 'spatial_curve_numeric':case(SPATIAL,[4.5,3,2],[4.5,3,2],5),
 'curved_offbase':case(CURVED,[5,5,1],[6,3,4],[8,7,6]),
 'surface_domain_clamped':case(PLANE,[5,5,0],[12,3,2],[15,7,8]),
 'normal_cplane_independent':dict(case(CIRCLE,[4.99,.2,0],[3,4,0],[7,6,2]),construction_plane=[[10,20,30],[0,1,0],[0,0,1]]),
}
