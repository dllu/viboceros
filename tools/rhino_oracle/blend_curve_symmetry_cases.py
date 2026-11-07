"""Fixed public Shift selections; no native result enters prescribed input."""
def pilot(held,case,pick):
    def click(point,shift=True):return dict(click=point,shift=shift)
    result={
      'symmetric_shape_baseline':held(extra=[click([1.8,0.,0.],False),[3.,0.,0.]]),
      'symmetric_shape_start':held(extra=[click([1.8,0.,0.]),[3.,0.,0.]]),
      'symmetric_shape_end':held(extra=[click([8.2,3.,0.]),[7.,3.,0.]]),
      'symmetric_mixed_shape':held(1,2,extra=[click([3.1,0.,0.]),[4.,0.,0.]]),
      'symmetric_contact_start':held(extra=[click([0.,0.,0.]),[-1.,0.,0.]]),
      'symmetric_contact_end':held(extra=[click([10.,3.,0.]),[11.,3.,0.]]),
      'symmetric_unequal_handles':held(extra=[click([8.2,3.,0.],False),[6.,3.,0.],click([1.8,0.,0.]),[3.,0.,0.]]),
      'symmetric_point_contact':case(['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.],'Trim=No','Join=No',click([0.,0.,0.]),[-1.,0.,0.],'Enter']),
    }
    for v in result.values():v['measure_jets']=True
    return result

def discovery(held,case,pick,arc,line,rational,quintic):
    def click(point,shift=True):return dict(click=point,shift=shift)
    vertical=dict(type='line',start=[10.,3.,0.],end=[10.,8.,0.])
    result={
      'symmetric_g1':held(1,1,extra=[click([3.1,0.,0.]),[4.,0.,0.]]),
      'symmetric_g3':held(3,3,extra=[click([1.3,0.,0.]),[2.5,0.,0.]]),
      'symmetric_g4_end':held(4,4,extra=[click([9.,3.,0.]),[7.5,3.,0.]]),
      'symmetric_mixed_g3_g4':held(3,4,extra=[click([1.2,0.,0.]),[2.5,0.,0.]]),
      'symmetric_perpendicular':held(curves=[dict(type='line',start=[-5.,0.,0.],end=[0.,0.,0.]),vertical],extra=[click([3.,0.,0.]),[4.,0.,0.]]),
      'symmetric_reversed':held(picks=[[-4.75,0.,0.],[14.75,3.,0.]],extra=[click([-8.5,0.,0.]),[-10.,0.,0.]]),
      'symmetric_arc':held(curves=[arc,vertical],picks=[[-.25,-.006,0.],[10.,3.25,0.]],extra=[click([3.,0.,0.]),[4.,0.,0.]]),
      'symmetric_rational_g3':held(3,3,curves=[rational,line],extra=[click([1.2,0.,.4]),[3.,0.,1.]]),
      'symmetric_reverse_quintic_g4':held(4,4,curves=[quintic,line],picks=[[-9.75,-3.8,1.],[14.75,3.,0.]],extra=[click([-11.,-5.,.5]),[-12.,-6.,0.]]),
      'symmetric_point_shape':case(['BlendStart=CurveEnd','Point',[10.,3.,0.],pick(0,[-.25,0.,0.]),'Trim=No','Join=No',click([3.4,0.,0.]),[4.,0.,0.],'Enter']),
      'symmetric_g0_peer':held(0,2,extra=[click([7.,3.,0.]),[6.,3.,0.]]),
      'symmetric_then_independent':held(extra=[click([1.8,0.,0.]),[3.,0.,0.],click([7.,3.,0.],False),[6.,3.,0.]]),
      'symmetric_pointer_target':held(extra=[click([1.8,0.,0.]),click([3.,0.,0.],False)]),
      'symmetric_portions_join':case(['BlendStart=CurveEnd','SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],[-1.,0.,0.],'SubCrv',pick(1,[10.25,3.,0.]),[11.,3.,0.],[14.,3.,0.],'Trim=Yes','Join=Yes',click([1.5,0.,0.]),[3.,0.,0.],'Enter']),
    }
    for v in result.values():v['measure_jets']=True
    return result

def holdouts(held,case,pick,quartic,rational,cubic_a,cubic_b):
    def click(point,shift=True):return dict(click=point,shift=shift)
    result={
      'symmetric_quartic_g4':held(4,4,curves=[quartic,dict(type='line',start=[10.,3.,0.],end=[15.,3.,0.])],extra=[click([1.,0.,0.]),[2.5,0.,0.]]),
      'symmetric_mixed_reverse':held(2,1,extra=[click([7.7,3.,0.]),[6.,3.,0.]]),
      'symmetric_unequal_rational':held(3,3,curves=[rational,dict(type='line',start=[10.,3.,0.],end=[15.,3.,0.])],extra=[click([1.2,0.,.4],False),[3.,0.,1.],click([8.7,3.,0.]),[6.,3.,0.]]),
      'symmetric_contact_after_shape':held(extra=[click([1.8,0.,0.]),[3.,0.,0.],click([0.,0.,0.]),[-1.,0.,0.]]),
      'symmetric_new_selection_independent':held(extra=[click([1.8,0.,0.],False),[3.,0.,0.]]),
      'symmetric_pointer_release':held(extra=[click([1.8,0.,0.]),click([4.,0.,0.],False)]),
      'symmetric_cubic_join':held(curves=[cubic_a,cubic_b],extra=[click([8.2,3.,0.]),[7.,3.,0.],'Trim=Yes','Join=Yes']),
      'symmetric_g4_join':held(4,4,extra=[click([1.,0.,0.]),[2.5,0.,0.],'Trim=Yes','Join=Yes']),
    }
    for v in result.values():v['measure_jets']=True
    return result
