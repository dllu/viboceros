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
RADIUS_TARGETS={}
for name,pose,targets in [('parallel',S_SHAPE,[-15.,-5.,-1.,0.,1.,5.,15.]),('corner',BASE,[-10.,-5.,-2.,-1.,0.,1.,2.5,3.,5.,10.]),('skew',BALANCE_POSE,[-3.,-1.,0.,1.,3.]),('spatial',SPATIAL,[-5.,-1.,0.,1.,5.])]:
 for i,target in enumerate(targets):
  RADIUS_TARGETS['rd_'+name+'_'+str(i)]=dict(recipe(*pose,options=['Trim=No','RadiusDifference',target]),measure_arcs=True)
RADIUS_TARGETS['rd_corner_from_s']=dict(recipe(*BASE,options=[dict(click=chord(*BASE)),.05,'RadiusDifference',0.]),measure_arcs=True)
CASES.update(RADIUS_TARGETS)
RADIUS_PARALLEL={
 'rd_parallel_shifted':recipe([100.,-80.,7.],[3.,4.,0.],[111.,-72.,10.],[6.,8.,0.],options=['Trim=No','RadiusDifference',2.]),
 'rd_parallel_scaled':recipe([0.,0.,0.],[50.,0.,0.],[100.,30.,0.],[50.,0.,0.],options=['RadiusDifference',-50.]),
 'rd_parallel_reversed':recipe([0.,0.,0.],[-5.,0.,0.],[-10.,-3.,0.],[-5.,0.,0.],options=['RadiusDifference',-5.]),
 'rd_parallel_unreachable_low':recipe(*S_SHAPE,options=['RadiusDifference',-30.]),
 'rd_parallel_unreachable_high':recipe(*S_SHAPE,options=['RadiusDifference',30.]),
 'rd_parallel_contact':recipe(*S_SHAPE,options=['RadiusDifference',5.,dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'rd_parallel_override':recipe(*S_SHAPE,options=['RadiusDifference',5.,dict(click=chord(*S_SHAPE,ratio=139./79.)),.25]),
 'rd_parallel_end_zero':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),dict(click=[0.,0.,0.])]),
 'rd_parallel_typed_zero':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),.25,dict(click=chord(*S_SHAPE,ratio=.25)),0.]),
}
RADIUS_PARALLEL={k:dict(v,measure_arcs=True)for k,v in RADIUS_PARALLEL.items()}
CASES.update(RADIUS_PARALLEL)
RADIUS_RECALL={
 'rd_zero_explicit':recipe(*S_SHAPE,options=['RadiusDifference',0.,dict(click=chord(*S_SHAPE)),.25,dict(click=chord(*S_SHAPE,ratio=.25)),0.]),
 'rd_snap_explicit':recipe(*S_SHAPE,options=['RadiusDifference',0.,dict(click=chord(*S_SHAPE)),dict(click=[0.,0.,0.])]),
 'rd_recall_typed':recipe(*S_SHAPE,options=['RadiusDifference',5.,dict(click=chord(*S_SHAPE,ratio=139./79.)),.25,dict(click=chord(*S_SHAPE,ratio=.25)),0.]),
 'rd_recall_snap':recipe(*S_SHAPE,options=['RadiusDifference',5.,dict(click=chord(*S_SHAPE,ratio=139./79.)),.25,dict(click=chord(*S_SHAPE,ratio=.25)),dict(click=[0.,0.,0.])]),
}
RADIUS_RECALL={k:dict(v,measure_arcs=True)for k,v in RADIUS_RECALL.items()}
CASES.update(RADIUS_RECALL)
RADIUS_GENERAL={}
for name,pose,targets in [('skew',BALANCE_POSE,[-.1,-.01,0.,.01,.1,2.,2.4,2.5,2.6,3.,4.,5.]),('spatial',SPATIAL,[-2.,-1.5,-.5,.5,1.5,2.,3.,4.]),('corner',BASE,[-1.5,-.5,.5,1.5,2.,2.4,2.6,3.5,4.,4.5])]:
 for i,target in enumerate(targets):
  RADIUS_GENERAL['rg_'+name+'_'+str(i)]=dict(recipe(*pose,options=['Trim=No','RadiusDifference',target]),measure_arcs=True)
for i,scale in enumerate([.1,10.]):
 pose=tuple([x*scale for x in v]for v in BALANCE_POSE)
 RADIUS_GENERAL['rg_skew_scale_'+str(i)]=dict(recipe(*pose,options=['Trim=No','RadiusDifference',0.]),measure_arcs=True)
CASES.update(RADIUS_GENERAL)
RADIUS_GENERAL_HOLDOUTS={}
for name,pose,targets in [('corner',BASE,[-1.,0.,1.,5.]),('skew',BALANCE_POSE,[-3.,0.]),('spatial',SPATIAL,[0.,1.])]:
 for i,target in enumerate(targets):
  RADIUS_GENERAL_HOLDOUTS['rg_alt_'+name+'_'+str(i)]=dict(recipe(*pose,options=['Trim=No','AlternateSolution','RadiusDifference',target]),measure_arcs=True)
RADIUS_GENERAL_HOLDOUTS.update({
 'rg_back_start':dict(recipe([0.,0.,0.],[-5.,0.,0.],[5.,4.,0.],[0.,5.,0.],options=['RadiusDifference',0.]),measure_arcs=True),
 'rg_back_both':dict(recipe([0.,0.,0.],[-5.,0.,0.],[5.,4.,0.],[0.,-5.,0.],options=['RadiusDifference',2.]),measure_arcs=True),
 'rg_contact_skew':dict(recipe(*BALANCE_POSE,options=['RadiusDifference',0.,dict(click=[10.,4.,0.]),[12.,3.,0.]]),measure_arcs=True),
 'rg_contact_spatial':dict(recipe(*SPATIAL,options=['RadiusDifference',1.,dict(click=[7.,4.,3.]),[6.5,4.5,4.]]),measure_arcs=True),
 'rg_trim_skew':dict(recipe(*BALANCE_POSE,options=['Trim=Yes','Join=No','RadiusDifference',-1.,dict(click=[0.,0.,0.]),[-2.,0.,0.]]),measure_arcs=True),
})
for i,scale in enumerate([.1,10.]):
 pose=tuple([x*scale for x in v]for v in BASE)
 RADIUS_GENERAL_HOLDOUTS['rg_corner_scale_'+str(i)]=dict(recipe(*pose,options=['Trim=No','RadiusDifference',scale]),measure_arcs=True)
CASES.update(RADIUS_GENERAL_HOLDOUTS)
RADIUS_COCIRCULAR={}
for i,target in enumerate([0.,1.,-1.]):
 RADIUS_COCIRCULAR['rg_cocircular_'+str(i)]=dict(recipe([0.,0.,0.],[5.,0.,0.],[5.,5.,0.],[0.,5.,0.],options=['Trim=No','RadiusDifference',target]),measure_arcs=True)
RADIUS_COCIRCULAR['rg_cocircular_alt']=dict(recipe([0.,0.,0.],[5.,0.,0.],[5.,5.,0.],[0.,5.,0.],options=['Trim=No','AlternateSolution','RadiusDifference',0.]),measure_arcs=True)
CASES.update(RADIUS_COCIRCULAR)
RADIUS_GETTERS={
 'radius_getter_option':recipe(*S_SHAPE,options=['Trim=No',dict(click=chord(*S_SHAPE)),'RadiusDifference',5.,0.]),
 'radius_getter_option_then_ratio':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),'RadiusDifference',5.,.25]),
 'radius_main_default':recipe(*S_SHAPE,options=['RadiusDifference',5.,dict(click=chord(*S_SHAPE,ratio=139./79.)),.25,'RadiusDifference','Enter']),
 'radius_getter_default':recipe(*S_SHAPE,options=['RadiusDifference',5.,dict(click=chord(*S_SHAPE,ratio=139./79.)),.25,dict(click=chord(*S_SHAPE,ratio=.25)),'RadiusDifference','Enter',0.]),
 'radius_spatial_getter':recipe(*SPATIAL,options=[dict(click=chord(*SPATIAL)),'RadiusDifference',5.,0.]),
}
RADIUS_GETTERS={k:dict(v,measure_arcs=True)for k,v in RADIUS_GETTERS.items()}
CASES.update(RADIUS_GETTERS)
RADIUS_GETTER_HOLDOUTS={
 'radius_option_then_enter':recipe(*S_SHAPE,options=['Trim=No',dict(click=chord(*S_SHAPE)),'RadiusDifference',5.,'Enter']),
 'radius_active_option_then_enter':recipe(*S_SHAPE,options=['RadiusDifference',5.,dict(click=chord(*S_SHAPE,ratio=139./79.)),'RadiusDifference',-5.,'Enter']),
 'radius_default_zero':recipe(*S_SHAPE,options=['RadiusDifference',0.,dict(click=chord(*S_SHAPE)),.25,'RadiusDifference','Enter']),
 'radius_nested_negative':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),'RadiusDifference',-5.,0.]),
 'circle_repeat_positive':recipe([0.,0.,0.],[5.,0.,0.],[5.,5.,0.],[0.,5.,0.],options=['RadiusDifference',1.]),
 'circle_repeat_negative':recipe([0.,0.,0.],[5.,0.,0.],[5.,5.,0.],[0.,5.,0.],options=['RadiusDifference',-1.]),
}
RADIUS_GETTER_HOLDOUTS={k:dict(v,measure_arcs=True)for k,v in RADIUS_GETTER_HOLDOUTS.items()}
CASES.update(RADIUS_GETTER_HOLDOUTS)
PREFERENCES={
 'prefs_initial':recipe(*S_SHAPE),
 'prefs_radius_commit':recipe(*S_SHAPE,options=['RadiusDifference',5.]),
 'prefs_manual_reset':recipe(*S_SHAPE),
 'prefs_radius_default':recipe(*S_SHAPE,options=['RadiusDifference','Enter']),
 'prefs_radius_cancel':recipe(*S_SHAPE,options=['RadiusDifference',-5.]),
 'prefs_radius_after_cancel':recipe(*S_SHAPE,options=['RadiusDifference','Enter']),
 'prefs_join_commit':recipe(*S_SHAPE,options=['Trim=Yes','Join=Yes',dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'prefs_join_default':recipe(*S_SHAPE,options=[dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'prefs_trim_cancel':recipe(*S_SHAPE,options=['Trim=No']),
 'prefs_trim_after_cancel':recipe(*S_SHAPE,options=[dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'prefs_join_restored':recipe(*S_SHAPE,options=['Trim=Yes',dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'prefs_join_cancel':recipe(*S_SHAPE,options=['Join=No']),
 'prefs_join_after_cancel':recipe(*S_SHAPE,options=[dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
 'prefs_alternate_commit':recipe(*BASE,options=['Trim=No','AlternateSolution',dict(click=chord(*BASE)),.25]),
 'prefs_alternate_reset':recipe(*BASE),
}
for name in ['prefs_radius_cancel','prefs_trim_cancel','prefs_join_cancel']:
 PREFERENCES[name]['actions'][-1]='Cancel'
PREFERENCES={k:dict(v,measure_arcs=True)for k,v in PREFERENCES.items()}
CASES.update(PREFERENCES)
PREFERENCE_HOLDOUTS={
 'prefs_nested_cancel':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),'RadiusDifference',5.,'Enter','Cancel']),
 'prefs_zero_recall':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),0.]),
 'prefs_numeric_cancel':recipe(*S_SHAPE,options=['RadiusDifference','Cancel']),
 'prefs_numeric_default':recipe(*S_SHAPE,options=['RadiusDifference','Enter']),
 'prefs_negative_cancel':recipe(*S_SHAPE,options=['RadiusDifference',-5.,'Cancel']),
 'prefs_zero_after_cancel':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),0.]),
 'prefs_pending_cancel':dict(recipe(*S_SHAPE,options=['Trim=Yes','Join=Yes','Cancel']),pending_redo=True),
 'prefs_join_after_pending':recipe(*S_SHAPE,options=[dict(click=[0.,0.,0.]),[-2.,0.,0.]]),
}
for name in ['prefs_nested_cancel','prefs_numeric_cancel','prefs_negative_cancel','prefs_pending_cancel']:
 PREFERENCE_HOLDOUTS[name]['actions'].pop()
PREFERENCE_HOLDOUTS={k:dict(v,measure_arcs=True)for k,v in PREFERENCE_HOLDOUTS.items()}
CASES.update(PREFERENCE_HOLDOUTS)
PREFERENCE_CANCELLATION={
 'prefs_numeric_seed':recipe(*S_SHAPE,options=['RadiusDifference',5.]),
 'prefs_numeric_twice':recipe(*S_SHAPE,options=['RadiusDifference','Cancel','Cancel']),
 'prefs_zero_after_numeric':recipe(*S_SHAPE,options=[dict(click=chord(*S_SHAPE)),0.]),
}
PREFERENCE_CANCELLATION['prefs_numeric_twice']['actions'].pop()
PREFERENCE_CANCELLATION={k:dict(v,measure_arcs=True)for k,v in PREFERENCE_CANCELLATION.items()}
CASES.update(PREFERENCE_CANCELLATION)
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='arc_blend_command'or not isinstance(op.get('case'),str)or op.get('case')not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed ArcBlend command recipe')
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='arc_blend_command',id=k,case=k)for k in(PILOT if names is None else names)])
