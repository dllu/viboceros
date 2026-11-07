"""Owned command discovery for picked ArcBlend curve ends and options."""
def line(a,b):return dict(type='line',start=a,end=b)
def recipe(p,t0,q,t1,options=None):
 return dict(curves=[line([p[i]-t0[i]for i in range(3)],p),line(q,[q[i]+t1[i]for i in range(3)])],view='Top',actions=[dict(curve=0,pick=[p[i]-.05*t0[i]for i in range(3)]),dict(curve=1,pick=[q[i]+.05*t1[i]for i in range(3)])]+(options or[])+['Enter'])
BASE=([0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,5.,0.])
CASES={
 'arc_default':recipe(*BASE),
 'arc_s_shape':recipe([0.,0.,0.],[5.,0.,0.],[10.,3.,0.],[5.,0.,0.]),
 'arc_spatial':recipe([0.,0.,0.],[1.,2.,-1.],[7.,4.,3.],[-1.,1.,2.]),
 'arc_alternate':recipe(*BASE,options=['AlternateSolution']),
 'arc_radius_difference':recipe(*BASE,options=['RadiusDifference',0.]),
 'arc_unjoined':recipe(*BASE,options=['Trim=No','Join=No']),
}
PILOT=list(CASES)
HOLDOUTS={
 'arc_radius_zero':recipe(*BASE,options=['RadiusDifference',0.]),
 'arc_radius_positive':recipe(*BASE,options=['RadiusDifference',1.]),
 'arc_radius_negative':recipe(*BASE,options=['RadiusDifference',-1.]),
 'arc_trim':recipe(*BASE,options=['Trim=Yes']),
 'arc_join':recipe(*BASE,options=['Trim=Yes','Join=Yes']),
 'arc_ratio_direct':recipe(*BASE,options=[.25]),
 'arc_ratio_zero':recipe(*BASE,options=[0.]),
}
CASES.update(HOLDOUTS)
ADJUSTMENTS={
 'arc_handle_start':recipe(*BASE,options=[dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'arc_handle_end':recipe(*BASE,options=[dict(click=[5.,4.,0.]),[5.,6.,0.]]),
 'arc_handle_ratio_quarter':recipe(*BASE,options=[dict(click=[3.44,1.06,0.]),.25]),
 'arc_handle_ratio_two':recipe(*BASE,options=[dict(click=[3.44,1.06,0.]),2.]),
 'arc_handle_ratio_zero':recipe(*BASE,options=[dict(click=[3.44,1.06,0.]),0.]),
 'arc_handle_point':recipe(*BASE,options=[dict(click=[3.44,1.06,0.]),[3.,1.,0.]]),
}
CASES.update(ADJUSTMENTS)
def joint(p,t0,q,t1,ratio=1.,alternate=False):
 from math import sqrt
 normalize=lambda v:[x/sqrt(sum(y*y for y in v))for x in v]
 t0,t1=normalize(t0),normalize(t1);v=[q[i]-p[i]for i in range(3)]
 a=ratio/(1.+ratio);b=1.-a;delta=[t0[i]-t1[i]for i in range(3)]
 A=a*b*sum(x*x for x in delta);B=2.*sum(v[i]*(a*t0[i]+b*t1[i])for i in range(3));C=sum(x*x for x in v)
 d=C/B if A==0. else(-B+(-1. if alternate else 1.)*sqrt(B*B+4*A*C))/(2*A)
 return[p[i]+a*v[i]+a*b*d*delta[i]for i in range(3)]
SPATIAL=([0.,0.,0.],[1.,2.,-1.],[7.,4.,3.],[-1.,1.,2.])
S_SHAPE=([0.,0.,0.],[5.,0.,0.],[10.,3.,0.],[5.,0.,0.])
HANDLE_HOLDOUTS={
 'arc_start_offcurve':recipe(*BASE,options=[dict(click=[0.,0.,0.]),[-2.,2.,1.]]),
 'arc_end_offcurve':recipe(*BASE,options=[dict(click=[5.,4.,0.]),[6.,6.,2.]]),
 'arc_start_clamp':recipe(*BASE,options=[dict(click=[0.,0.,0.]),[-20.,0.,0.]]),
 'arc_handle_trim':recipe(*BASE,options=['Trim=Yes','Join=No',dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'arc_handle_join':recipe(*BASE,options=['Trim=Yes','Join=Yes',dict(click=[0.,0.,0.]),[-2.,0.,0.],dict(click=[5.,4.,0.]),[5.,6.,0.]]),
 'arc_ratio_chord':recipe(*BASE,options=['Trim=No',dict(click=joint(*BASE)),[1.,3.,0.]]),
 'arc_ratio_offplane':recipe(*BASE,options=[dict(click=joint(*BASE)),[4.,2.,6.]]),
 'arc_ratio_spatial':recipe(*SPATIAL,options=[dict(click=joint(*SPATIAL)),[2.,3.,5.]]),
 'arc_ratio_alternate':recipe(*BASE,options=['AlternateSolution',dict(click=joint(*BASE,alternate=True)),.25]),
 'arc_ratio_s_quarter':recipe(*S_SHAPE,options=[dict(click=joint(*S_SHAPE)),.25]),
 'arc_zero_s_after_quarter':recipe(*S_SHAPE,options=[dict(click=joint(*S_SHAPE)),.25,dict(click=joint(*S_SHAPE,ratio=.25)),0.]),
 'arc_zero_corner_after_two':recipe(*BASE,options=[dict(click=joint(*BASE)),2.,dict(click=joint(*BASE,ratio=2.)),0.]),
 'arc_mouse_ratio_top':recipe(*BASE,options=[dict(click=joint(*BASE)),dict(click=[3.,1.,0.])]),
 'arc_mouse_ratio_spatial':recipe(*SPATIAL,options=[dict(click=joint(*SPATIAL)),dict(click=[2.,3.,5.])]),
 'arc_mouse_ratio_perspective':dict(recipe(*SPATIAL,options=[dict(click=joint(*SPATIAL)),dict(click=[2.,3.,5.])]),view='Perspective'),
}
CASES.update(HANDLE_HOLDOUTS)
BALANCE_POSE=([0.,0.,0.],[5.,0.,0.],[10.,4.,0.],[4.,-2.,0.])
BALANCE={
 'arc_auto_skew_tangents':recipe(*BALANCE_POSE,options=[dict(click=joint(*BALANCE_POSE)),0.]),
 'arc_auto_skew_after_two':recipe(*BALANCE_POSE,options=[dict(click=joint(*BALANCE_POSE)),2.,dict(click=joint(*BALANCE_POSE,ratio=2.)),0.]),
 'arc_auto_skew_contact':recipe(*BALANCE_POSE,options=[dict(click=joint(*BALANCE_POSE)),0.,dict(click=[10.,4.,0.]),[12.,3.,0.]]),
 'arc_auto_skew_chord_end':recipe(*BALANCE_POSE,options=[dict(click=joint(*BALANCE_POSE)),dict(click=[0.,0.,0.])]),
 'arc_radius_skew_zero':recipe(*BALANCE_POSE,options=['RadiusDifference',0.]),
 'arc_auto_spatial':dict(recipe(*SPATIAL,options=[dict(click=joint(*SPATIAL)),0.]),view='Perspective'),
 'arc_ratio_spatial_typed_perspective':dict(recipe(*SPATIAL,options=[dict(click=joint(*SPATIAL)),[2.,3.,5.]]),view='Perspective'),
 'arc_spatial_hover_pick':recipe(*SPATIAL,options=[dict(hover=joint(*SPATIAL)),dict(click=joint(*SPATIAL)),.25]),
 'arc_alternate_old_point':recipe(*BASE,options=['AlternateSolution',dict(click=joint(*BASE)),.25]),
 'arc_alternate_hover_point':recipe(*BASE,options=['AlternateSolution',dict(hover=joint(*BASE,alternate=True)),dict(click=joint(*BASE,alternate=True)),.25]),
}
CASES.update(BALANCE)
def chord(p,t0,q,t1,ratio=1.):return[p[i]+ratio/(1.+ratio)*(q[i]-p[i])for i in range(3)]
CHORD_HANDLES={
 'arc_chord_spatial_typed':recipe(*SPATIAL,options=[dict(click=chord(*SPATIAL)),[2.,3.,5.]]),
 'arc_chord_spatial_mouse':recipe(*SPATIAL,options=[dict(click=chord(*SPATIAL)),dict(click=[2.,3.,5.])]),
 'arc_chord_alternate_ratio':recipe(*BASE,options=['AlternateSolution',dict(click=chord(*BASE)),.25]),
 'arc_chord_spatial_perspective_ratio':dict(recipe(*SPATIAL,options=[dict(click=chord(*SPATIAL)),.25]),view='Perspective'),
 'arc_chord_s_typed':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),[8.,1.,0.]]),
 'arc_chord_skew_typed':recipe(*BALANCE_POSE,options=[dict(click=chord(*BALANCE_POSE)),[3.,2.,1.]]),
 'arc_chord_spatial_perspective_mouse':dict(recipe(*SPATIAL,options=[dict(click=chord(*SPATIAL)),dict(click=[2.,3.,5.])]),view='Perspective'),
}
CASES.update(CHORD_HANDLES)
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='arc_blend_command'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed ArcBlend command recipe')
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='arc_blend_command',id=k,case=k)for k in(PILOT if names is None else names)])
