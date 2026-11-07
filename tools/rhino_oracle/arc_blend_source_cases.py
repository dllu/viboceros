"""Prescribed curved guides and contacts, independent of native blend outputs."""

def nurbs(points,weights=None):
    weights=weights or [1.]*len(points)
    return dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=w)for p,w in zip(points,weights)],knots=[0.,0.,0.,0.,1.,1.,1.,1.])

def ordered_sum(values):
    # Python 3's compensated float sum differs from IronPython's fold. Keep
    # prescribed input points bit-for-bit reproducible in both runtimes.
    total=0.
    for value in values:total+=value
    return total

def bezier(curve,t):
    controls=curve['control_points'];basis=[b*c['weight']for b,c in zip([(1-t)**3,3*(1-t)**2*t,3*(1-t)*t*t,t**3],controls)]
    return [ordered_sum(b*c['point'][j]for b,c in zip(basis,controls))/ordered_sum(basis)for j in range(3)]

def picked(curves,picks,options=(),view='Top'):
    return dict(curves=curves,view=view,measure_arcs=True,actions=[dict(curve=i,pick=p)for i,p in enumerate(picks)]+['Trim=No']+list(options)+['Enter'])

A=nurbs([[-6.,-3.,0.],[-4.,-3.,0.],[-3.,0.,0.],[0.,0.,0.]])
B=nurbs([[10.,3.,0.],[13.,3.,0.],[14.,6.,0.],[16.,7.,0.]])
PICKS=[bezier(A,.98),bezier(B,.02)]
START=[dict(click=[0.,0.,0.]),bezier(A,.6)]
END=[dict(click=[10.,3.,0.]),bezier(B,.4)]
CURVED_SOURCES={
 'source_cubic_default':picked([A,B],PICKS),
 'source_cubic_start':picked([A,B],PICKS,START),
 'source_cubic_end':picked([A,B],PICKS,END),
 'source_cubic_trim':picked([A,B],PICKS,['Trim=Yes','Join=No']+START+END),
 'source_cubic_join':picked([A,B],PICKS,['Trim=Yes','Join=Yes']+START+END),
 'source_cubic_radius':picked([A,B],PICKS,['RadiusDifference',1.]+START),
}
C=nurbs([[-6.,-3.,2.],[-4.,-3.,1.],[-3.,0.,-1.],[0.,0.,0.]],[1.,2.,.5,1.])
D=nurbs([[8.,4.,3.],[9.,5.,4.],[13.,4.,2.],[15.,7.,1.]],[1.,.75,1.5,1.])
CURVED_SOURCES.update({
 'source_rational_spatial':picked([C,D],[bezier(C,.98),bezier(D,.02)],[dict(click=[0.,0.,0.]),bezier(C,.65)],view='Perspective'),
 'source_rational_ratio':picked([C,D],[bezier(C,.98),bezier(D,.02)],[dict(click=[4.,2.,1.5]),.25],view='Perspective'),
})
ARC_A=dict(type='arc',points=[[-5.,-5.,0.],[-3.,-1.,0.],[0.,0.,0.]])
ARC_B=dict(type='arc',points=[[10.,3.,0.],[13.,4.,0.],[15.,7.,0.]])
ARC_START=[dict(click=[0.,0.,0.]),[-3.,-1.,0.]]
ARC_END=[dict(click=[10.,3.,0.]),[13.,4.,0.]]
CURVED_SOURCES.update({
 'source_arc_default':picked([ARC_A,ARC_B],[[0.,0.,0.],[10.,3.,0.]]),
 'source_arc_start':picked([ARC_A,ARC_B],[[0.,0.,0.],[10.,3.,0.]],ARC_START),
 'source_arc_join':picked([ARC_A,ARC_B],[[0.,0.,0.],[10.,3.,0.]],['Trim=Yes','Join=Yes']+ARC_START+ARC_END),
 'source_polycurve_trim':picked([dict(type='polycurve',segments=[dict(type='line',start=[-10.,-3.,0.],end=[-6.,-3.,0.]),A]),B],PICKS,['Trim=Yes','Join=No']+START),
 'source_polyline_corner':picked([dict(type='polyline',vertices=[[-5.,-3.,0.],[-2.,0.,0.],[0.,0.,0.],[0.,2.,0.]]),B],[[0.,1.9,0.],bezier(B,.02)],[dict(click=[0.,2.,0.]),[0.,0.,0.]]),
})

POLY_ARC=dict(type='polycurve',segments=[dict(type='line',start=[-10.,-5.,0.],end=[-5.,-5.,0.]),ARC_A])
SOURCE_HOLDOUTS={
 'source_arc_mouse':picked([ARC_A,ARC_B],[[0.,0.,0.],[10.,3.,0.]],[dict(click=[0.,0.,0.]),dict(click=[-3.,-1.,0.])]),
 'source_arc_mouse_join':picked([ARC_A,ARC_B],[[0.,0.,0.],[10.,3.,0.]],['Trim=Yes','Join=Yes',dict(click=[0.,0.,0.]),dict(click=[-3.,-1.,0.]),dict(click=[10.,3.,0.]),dict(click=[13.,4.,0.])]),
 'source_polycurve_arc':picked([POLY_ARC,ARC_B],[[0.,0.,0.],[10.,3.,0.]],ARC_START),
 'source_polycurve_arc_mouse':picked([POLY_ARC,ARC_B],[[0.,0.,0.],[10.,3.,0.]],[dict(click=[0.,0.,0.]),dict(click=[-3.,-1.,0.])]),
 'source_arc_clamp':picked([ARC_A,ARC_B],[[0.,0.,0.],[10.,3.,0.]],[dict(click=[0.,0.,0.]),[-50.,-50.,0.]]),
}
