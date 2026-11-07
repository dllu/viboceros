"""Owned hidden SubCrv selections, original parents and final ArcBlend edits."""
def line(a,b):return dict(type='line',start=a,end=b)
A=line([-5.,0.,0.],[0.,0.,0.]);B=line([10.,3.,0.],[15.,3.,0.])
def pick(i,p):return dict(curve=i,pick=p)
def first(start=None,end=None,near=None):
 return ['SubCrv',pick(0,near or[-.25,0.,0.]),start or[-4.,0.,0.],end or[-1.,0.,0.]]
def second():return ['SubCrv',pick(1,[10.25,3.,0.]),[11.,3.,0.],[14.,3.,0.]]
def case(actions,curves=None,**kwargs):
 return dict(curves=curves or[A,B],view='Top',actions=actions,measure_arcs=True,**kwargs)
SUBCURVE_DISCOVERY={
 'subcurve_first':case(first()+[pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_first_near_start':case(first(near=[-4.75,0.,0.])+[pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_reverse':case(first(start=[-1.,0.,0.],end=[-4.,0.,0.])+[pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_second':case([pick(0,[-.25,0.,0.])]+second()+['Trim=No','Enter']),
 'subcurve_both_trim':case(first()+second()+['Trim=Yes','Join=No','Enter']),
 'subcurve_join_slide':case(first()+[pick(1,[10.25,3.,0.]),'Trim=Yes','Join=Yes',dict(click=[-1.,0.,0.]),[-2.,0.,0.],'Enter']),
 'subcurve_circle':case(['SubCrv',pick(0,[5.,0.,0.]),[5.,0.,0.],[0.,5.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Enter'],curves=[dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.]),B]),
 'subcurve_cancel':case(['SubCrv','Cancel','Cancel'],pending_redo=True),
}

if __package__:from .arc_blend_source_cases import ARC_A,A as CUBIC_A,B as CUBIC_B,bezier
else:from arc_blend_source_cases import ARC_A,A as CUBIC_A,B as CUBIC_B,bezier
PARENT=dict(type='polyline',vertices=[[-5.,0.,0.],[0.,0.,0.],[10.,3.,0.],[15.,3.,0.]])
CIRCLE=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
SUBCURVE_HOLDOUTS={
 'subcurve_reverse_trim':case(first(start=[-1.,0.,0.],end=[-4.,0.,0.])+[pick(1,[10.25,3.,0.]),'Trim=Yes','Join=No','Enter']),
 'subcurve_clamp':case(first()+[pick(1,[10.25,3.,0.]),'Trim=No',dict(click=[-1.,0.,0.]),[0.,0.,0.],'Enter']),
 'subcurve_arc':case(['SubCrv',pick(0,[-5.,-5.,0.]),[-3.,-1.,0.],[0.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=No',dict(click=[0.,0.,0.]),[-3.,-1.,0.],'Enter'],curves=[ARC_A,B]),
 'subcurve_cubic_join':case(['SubCrv',pick(0,bezier(CUBIC_A,.02)),bezier(CUBIC_A,.2),bezier(CUBIC_A,.8),pick(1,bezier(CUBIC_B,.02)),'Trim=Yes','Join=Yes','Enter'],curves=[CUBIC_A,CUBIC_B]),
 'subcurve_circle_wrap':case(['SubCrv',pick(0,[5.,0.,0.]),[0.,5.,0.],[5.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Enter'],curves=[CIRCLE,B]),
 'subcurve_self_join':case(first()+['SubCrv',pick(0,[14.,3.,0.]),[11.,3.,0.],[14.,3.,0.],'Trim=Yes','Join=Yes','Enter'],curves=[PARENT]),
 'subcurve_mixed_self_join':case([pick(0,[14.75,3.,0.])]+first()+['Trim=Yes','Join=Yes','Enter'],curves=[PARENT]),
 'subcurve_numeric':case(['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],2.,[-1.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_negative_length':case(['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],-2.,[-1.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_zero':case(['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],0.,pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_locked':case(['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],dict(hover=[-1.,0.,0.]),'Direction=Locked',2.,pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_cancel_end':case(['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],'Cancel',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
}

SUBCURVE_POINTERS={
 'subcurve_mouse_line':case(['SubCrv',pick(0,[-.25,0.,0.]),dict(click=[-4.,0.,0.]),dict(click=[-1.,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
 'subcurve_mouse_arc':case(['SubCrv',pick(0,[-5.,-5.,0.]),dict(click=[-3.,-1.,0.]),dict(click=[0.,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Enter'],curves=[ARC_A,B]),
 'subcurve_mouse_circle':case(['SubCrv',pick(0,[5.,0.,0.]),dict(click=[5.,0.,0.]),dict(click=[0.,5.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Enter'],curves=[CIRCLE,B]),
 'subcurve_length_units':case(['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],dict(number='0.2cm'),[-1.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Enter'],model_units='Millimeters'),
 'subcurve_cancel_start':case(['SubCrv',pick(0,[-.25,0.,0.]),'Cancel',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Enter']),
}

SUBCURVE_CLOSED={
 'subcurve_circle_long':case(['SubCrv',pick(0,[5.,0.,0.]),[5.,0.,0.],[0.,-5.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Enter'],curves=[CIRCLE,B]),
 'subcurve_circle_reverse_join':case(['SubCrv',pick(0,[5.,0.,0.]),[0.,5.,0.],[5.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=Yes','Join=Yes','Enter'],curves=[CIRCLE,B]),
 'subcurve_circle_numeric_wrap':case(['SubCrv',pick(0,[5.,0.,0.]),[0.,5.,0.],25.,[5.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=Yes','Join=Yes','Enter'],curves=[CIRCLE,B]),
 'subcurve_circle_numeric_short':case(['SubCrv',pick(0,[5.,0.,0.]),[0.,5.,0.],5.,[5.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=Yes','Join=Yes','Enter'],curves=[CIRCLE,B]),
}
