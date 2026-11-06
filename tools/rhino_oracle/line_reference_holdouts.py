"""Independent endpoints, closed seams, spatial/remote guides and cancellation."""
if __package__:from .line_reference_cases import case,LINE,ARC,NURBS,POLYLINE
else:from line_reference_cases import case,LINE,ARC,NURBS,POLYLINE
CIRCLE=dict(type='circle',center=[0,0,0],radius=5,x_axis=[1,0,0],normal=[0,0,1])
HAIRPIN=dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=1)for p in [[0,0,0],[100,0,0],[100,1,0],[1,1,0]]],knots=[0,0,0,0,1,1,1,1])
REMOTE=dict(type='nurbs',degree=3,control_points=[dict(point=[1e12+p[0],1e12+p[1],1e12+p[2]],weight=1)for p in [[0,0,0],[2,3,0],[7,4,0],[10,0,0]]],knots=[0,0,0,0,1,1,1,1])
HOLDOUTS={
 'circle_seam':case(CIRCLE,[4.99,.2,0],[8,4,0]),
 'closed_polyline_end':case(dict(type='polyline',vertices=[[0,0,0],[4,0,0],[4,3,0],[0,0,0]]),[.2,.15,0],[-4,-3,0]),
 'hairpin_first_half':case(HAIRPIN,[48.008,.104,0],[4,-3,0]),
 'nurbs_remote':case(REMOTE,[1e12+9.5,1e12+.6,1e12],[1e12+13,1e12-4,1e12]),
 'end_reverse':case(LINE,[9,0,0],[6,7,2]),
 'nurbs_start':case(NURBS,[.2,.3,0],[-2,-3,0]),
 'polyline_start':case(POLYLINE,[.2,0,0],[-7,8,2]),
 'spatial_end':case(dict(type='line',start=[1,2,3],end=[5,6,7]),[4.8,5.8,6.8],[10,10,9]),
 'cplane_normal_ignored':dict(case(ARC,[.2,4.99,0],[-4,8,7]),construction_plane=[[10,20,30],[0,1,0],[0,0,1]]),
 'both_late':case(LINE,[9,0,0],'BothSides',[14,6,5]),
 'both_fixed_length':case(NURBS,[9.5,.6,0],'BothSides',5,[13,-4,0]),
 'coarse_tolerance':dict(case(LINE,[9,0,0],[10.0000001,2,3]),absolute_tolerance=.01),
 'redo_cancel':dict(case(LINE,[9,0,0],'Cancel'),pending_redo=True),
 'redo_empty':dict(actions=['Extension','Cancel'],pending_redo=True),
}
