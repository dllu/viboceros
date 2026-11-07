"""Prescribed point endpoints for the public adjustable blend command."""
def pilot(case,pick,a,b):
    suffix=['Trim=No','Join=No','Enter']
    return {
        'point_curve_to_point':case(['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.]]+suffix),
        'point_point_to_curve':case(['BlendStart=CurveEnd','Point',[0.,0.,0.],pick(1,[10.25,3.,0.])]+suffix),
        'point_two_points':case(['Point',[0.,0.,0.],'Point',[10.,3.,0.]]+suffix),
        'point_curve_g4':case(['Continuity_1=G4',pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.]]+suffix),
    }

def discovery(case,pick,line,quartic,cubic,unused):
    done=['Trim=No','Join=No','Enter']
    cases={}
    for order in range(5):
        cases['point_end_g'+str(order)]=case(['BlendStart=CurveEnd','Continuity_1=G'+str(order),pick(0,[-.25,0.,0.]),'Point',[10.,3.,2.]]+done,curves=[quartic,line],measure_jets=True)
    for order in range(1,5):
        cases['point_start_g'+str(order)]=case(['BlendStart=CurveEnd','Continuity_2=G'+str(order),'Point',[0.,0.,0.],pick(1,[10.25,3.,0.])]+done,curves=[unused,cubic],measure_jets=True)
    first=[pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.]]
    second=['Point',[0.,0.,0.],pick(1,[10.25,3.,0.])]
    cases.update({
        'point_move_end':case(first+['Trim=No','Join=No',dict(click=[10.,3.,0.]),[11.,5.,1.],'Enter']),
        'point_move_start':case(second+['Trim=No','Join=No',dict(click=[0.,0.,0.]),[-2.,1.,2.],'Enter']),
        'point_flip_curve':case(first+['Trim=No','Join=No','Flip',pick(0,[-.25,0.,0.]),'Enter']),
        'point_join_curve_first':case(first+['Trim=Yes','Join=Yes','Enter']),
        'point_join_point_first':case(second+['Trim=Yes','Join=Yes','Enter']),
        'point_cancel_prompt':case(['Point','Cancel'],pending_redo=True),
        'point_cancel_after_curve':case([pick(0,[-.25,0.,0.]),'Point','Cancel'],pending_redo=True),
    })
    return cases

def holdouts(case,pick,line,arc,rational,quintic):
    done=['Trim=No','Join=No','Enter']
    first=[pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.]]
    second=['Point',[0.,0.,0.],pick(1,[10.25,3.,0.])]
    interior=['BlendStart=PickPoint',pick(0,[-2.,0.,0.]),'Point',[10.,3.,0.]]
    cases={
        'point_arc_g2':case([pick(0,[-.25,-.006,0.]),'Point',[10.,3.,2.]]+done,curves=[arc,line]),
        'point_rational_g3':case(['Continuity_1=G3']+first+done,curves=[rational,line]),
        'point_interior_trim':case(interior+['Trim=Yes','Join=No','Enter']),
        'point_interior_join':case(interior+['Trim=Yes','Join=Yes','Enter']),
        'point_join_without_trim':case(second+['Trim=No','Join=Yes','Enter']),
        'point_curve_shape':case(first+['Trim=No','Join=No',dict(click=[3.48,0.,0.]),[4.,0.,0.],'Enter']),
        'point_slide_curve':case(first+['Trim=No','Join=No',dict(click=[0.,0.,0.]),[-1.,0.,0.],'Enter']),
        'point_flip_both':case(first+['Trim=No','Join=No','Flip','Enter','Enter']),
        'point_cancel_after_point':case(['Point',[0.,0.,0.],'Cancel'],pending_redo=True),
        'point_reverse_spatial_g4':case(['Continuity_1=G4',pick(0,[-9.75,-3.8,1.]),'Point',[12.,7.,8.]]+done,curves=[quintic,line]),
    }
    for value in cases.values():value['measure_jets']=True
    return cases

def ownership(case,pick):
    first=[pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.]]
    return {
        'point_curve_first_join_without_trim':case(['BlendStart=CurveEnd']+first+['Trim=No','Join=Yes','Enter'],measure_jets=True),
        'point_first_interior_join':case(['BlendStart=PickPoint','Point',[0.,0.,0.],pick(1,[12.,3.,0.]),'Trim=Yes','Join=Yes','Enter'],measure_jets=True),
        'point_g0_join_without_trim':case(['BlendStart=CurveEnd','Continuity_1=G0']+first+['Trim=No','Join=Yes','Enter'],measure_jets=True),
    }

def modes(case,pick):
    point_first=['Point',[0.,0.,0.],pick(1,[10.25,3.,0.])]
    curve_first=[pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.]]
    joined=['Trim=No','Join=Yes','Enter']
    cases={
        'point_natural_first_join':case(['BlendStart=CurveEnd']+point_first+joined),
        'point_natural_shape':case(['BlendStart=CurveEnd']+curve_first+['Trim=No','Join=No',dict(click=[3.48,0.,0.]),[4.,0.,0.],'Enter']),
        'point_interior_copy_join':case(['BlendStart=PickPoint',pick(0,[-2.,0.,0.]),'Point',[10.,3.,0.]]+joined),
        'point_two_interior_copy_join':case(['BlendStart=PickPoint',pick(0,[-2.,0.,0.]),pick(1,[12.,3.,0.])]+joined),
        'point_mixed_first_natural_join':case(['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),'BlendStart=PickPoint',pick(1,[12.,3.,0.])]+joined),
        'point_mixed_first_interior_join':case(['BlendStart=PickPoint',pick(0,[-2.,0.,0.]),'BlendStart=CurveEnd',pick(1,[10.25,3.,0.])]+joined),
    }
    for value in cases.values():value['measure_jets']=True
    return cases

def pick_context(case,pick,line,quintic):
    cases={}
    for name,mode,location in [
        ('spatial_pick_context_g4','PickPoint',[-9.75,-3.8,1.]),
        ('spatial_natural_g4','CurveEnd',[-9.75,-3.8,1.]),
        ('spatial_interior_g4','PickPoint',[-4.,-1.,1.]),
    ]:
        cases[name]=case(['BlendStart='+mode,'Continuity_1=G4',pick(0,location),'Point',[12.,7.,8.],'Trim=No','Join=No','Enter'],curves=[quintic,line],measure_jets=True)
    return cases
