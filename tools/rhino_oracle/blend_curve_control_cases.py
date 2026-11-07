"""Prescribed higher control positions for the minimum-degree blend net."""
if __package__:from .blend_curve_edge_cases import PLANE,OTHER,CURVED,edge
else:from blend_curve_edge_cases import PLANE,OTHER,CURVED,edge
def discovery(case):
    def click(p,shift=False):return dict(click=p,shift=shift)
    def pair(order,extra,curved=False):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+extra+['Enter'],curves=[CURVED if curved else PLANE,OTHER],measure_jets=True)
    result={}
    # Picks follow the analytic zero-tangential-jet default: k*h from the
    # contact. Only prescribed source geometry, never Native output, sets them.
    positions={2:{2:6.33},3:{2:4.52,3:6.78},4:{2:3.52,3:5.28,4:7.03}}
    for curved in [False,True]:
        for order in [2,3,4]:
            for control,x in positions[order].items():
                name='control_'+('curved_'if curved else'plane_')+'g'+str(order)+'_k'+str(control)
                result[name]=pair(order,[click([x,-2.46,0.]),[x+1.,0.,3.]],curved)
    result['control_plane_end_g2']=pair(2,[click([3.67,5.52,0.]),[6.,3.,0.]])
    result['control_plane_negative_g2']=pair(2,[click([6.33,-2.46,0.]),[-1.,0.,0.]])
    result['control_secondary_then_primary_g2']=pair(2,[click([3.67,5.52,0.]),[6.,3.,0.],click([6.83,5.52,0.]),[5.,5.5,0.]])
    result['control_secondary_then_primary_g3']=pair(3,[click([4.52,-2.46,0.]),[5.,0.,0.],click([2.26,-2.46,0.]),[3.,-2.5,0.]],True)
    result['control_secondary_then_contact']=pair(2,[click([6.33,-2.46,0.]),[7.,0.,0.],click([0.,-2.46,0.]),[0.,-1.,0.]],True)
    result['control_secondary_then_angle']=pair(2,[click([6.33,-2.46,0.]),[7.,0.,0.],'AtAngle_1=30'],True)
    result['control_secondary_shift']=pair(2,[click([6.33,-2.46,0.],True),[7.,0.,0.]])
    result['control_secondary_numeric']=pair(2,[click([6.33,-2.46,0.]),dict(number='4')])
    return result

def higher_policy(case):
    if __package__:from .blend_curve_edge_cases import tensor
    else:from blend_curve_edge_cases import tensor
    def profile(q,c):
        return tensor([-5.,-10./3.,-5./3.,0.],[-5.,0.],lambda i,j:q*[1.,1./3.,0.,0.][j]+c*[1.,0.,0.,0.][j])
    def pair(order,source,extra):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+extra+['Enter'],curves=[source,OTHER],measure_jets=True)
    result={}
    for name,q,c in [('quadratic',1.,0.),('cubic',0.,1.),('combined',1.,1.)]:
        for target in [4.,5.]:
            result['control_policy_'+name+'_'+str(int(target))]=pair(4,profile(q,c),[dict(click=[3.52,-2.46,0.]),[target,0.,3.]])
    result['control_policy_combined_second_third']=pair(4,profile(1.,1.),[dict(click=[3.52,-2.46,0.]),[4.5,0.,3.],dict(click=[6.26,-2.46,0.]),[7.5,0.,3.]])
    result['control_policy_combined_third']=pair(4,profile(1.,1.),[dict(click=[5.28,-2.46,0.]),[6.5,0.,3.]])
    for name,source in [('plane',PLANE),('curved',CURVED)]:
        result['control_numeric_'+name]=pair(2,source,[dict(click=[6.33,-2.46,0.]),dict(number='4'),dict(wait_prompt='Select control point to adjust')])
    return result

def calibration(case,pick,cubic,line):
    if __package__:from .blend_curve_edge_cases import tensor
    else:from blend_curve_edge_cases import tensor
    result={};wait=dict(wait_prompt='Select control point to adjust')
    def pair(source,extra,order=4):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+extra+[wait,'Enter'],curves=[source,OTHER],measure_jets=True)
    for height in [.5,2.,5.]:
        source=tensor([-5.,-10./3.,-5./3.,0.],[-5.,0.],lambda i,j:height*[1.,1./3.,0.,0.][j])
        for target in [4.,5.]:result['control_scale_'+str(height).replace('.','p')+'_'+str(int(target))]=pair(source,[dict(click=[3.52,-2.46,0.]),[target,0.,3.]])
    for order,control,position in [(3,2,[2.64,-.45,0.]),(4,2,[2.05,-.26,0.]),(4,3,[3.08,-.71,0.]),(4,4,[4.1,-1.27,0.])]:
        result['control_ordinary_g'+str(order)+'_k'+str(control)]=case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),dict(click=position),[position[0]+1.,0.,1.],wait,'Enter'],curves=[cubic,line],measure_jets=True)
    for name,source in [('plane',PLANE),('curved',CURVED)]:
        for target in [8.,-4.]:result['control_number_point_'+name+'_'+('forward'if target > 0. else'backward')]=pair(source,[dict(click=[6.33,-2.46,0.]),dict(number='4'),[target,0.,3.]],2)
    return result

def holdouts(case,pick,cubic,line):
    import copy,math
    result={};wait=dict(wait_prompt='Select control point to adjust')
    def pair(extra,order=4,reverse=False):
        sources=[edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]
        if reverse:sources.reverse()
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges']+sources+extra+[wait,'Enter'],curves=[CURVED,OTHER],measure_jets=True)
    for target in [2.,6.]:
        result['control_holdout_g4_second_'+str(int(target))]=pair([dict(click=[3.52,-2.46,0.]),[target,0.,1.]])
    result['control_holdout_g4_end']=pair([dict(click=[3.52,-2.46,0.]),[4.7,0.,1.]],reverse=True)
    result['control_holdout_g4_second_third']=pair([dict(click=[3.52,-2.46,0.]),[4.1,0.,1.],dict(click=[5.86,-2.46,0.]),[7.,0.,1.]])
    result['control_holdout_g4_primary']=pair([dict(click=[3.52,-2.46,0.]),[4.1,0.,1.],dict(click=[1.76,-2.46,0.]),[2.2,0.,1.]])
    result['control_holdout_g3_numeric']=pair([dict(click=[6.78,-2.46,0.]),dict(number='2.5'),[-1.,0.,2.]],3)
    result['control_holdout_g4_negative']=pair([dict(click=[3.52,-2.46,0.]),[-1.,0.,2.]])
    result['control_holdout_g4_shift']=pair([dict(click=[3.52,-2.46,0.],shift=True),[4.,0.,1.]])
    result['control_holdout_g3_shift_third']=pair([dict(click=[6.78,-2.46,0.],shift=True),[8.,0.,1.]],3)
    result['control_holdout_g4_angle']=pair([dict(click=[3.52,-2.46,0.]),[4.1,0.,1.],'AtAngle_1=25'])
    result['control_holdout_g4_contact']=pair([dict(click=[3.52,-2.46,0.]),[4.1,0.,1.],dict(click=[0.,-2.46,0.]),[0.,-1.,0.]])
    spatial=copy.deepcopy(result['control_holdout_g4_second_6']);spatial['view']='Perspective'
    def rotate(p):return [.8*p[0]+.6*p[2]+2.,p[1]+1.,-.6*p[0]+.8*p[2]-3.]
    for source in spatial['curves']:
        for p in source['control_points']:p['point']=rotate(p['point'])
    for a in spatial['actions']:
        if isinstance(a,dict):
            for key in ['pick','click']:
                if key in a:a[key]=rotate(a[key])
        elif isinstance(a,list):a[:]=rotate(a)
    result['control_holdout_spatial_g4']=spatial
    # Independent nonplanar cubic: prescribed endpoint derivatives give K,
    # and the default second control is 2h*T+K*(9h)^2/(9*8).
    torsion=copy.deepcopy(cubic)
    for p,z in zip(torsion['control_points'],[3.,2.,0.,0.]):p['point'][2]=z
    h=2./9.*(1.4*math.sqrt(109.)-10.)
    position=[2*h,-.25*(9*h)**2/72.,.15*(9*h)**2/72.]
    for reverse in [False,True]:
        sources=[pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.])]
        if reverse:sources.reverse()
        result['control_holdout_torsion_'+str(reverse)]=case(['Continuity_1=G4','Continuity_2=G4']+sources+[dict(click=position),[3.5,0.,1.],wait,'Enter'],curves=[torsion,line],measure_jets=True)
    cancelled=pair([dict(click=[3.52,-2.46,0.]),[4.1,0.,1.]])
    cancelled['actions'][-1]='Cancel';cancelled['pending_redo']=True;result['control_holdout_cancel']=cancelled
    return result

def validation(case,pick,cubic,line,rational):
    wait=dict(wait_prompt='Select control point to adjust')
    def pair(order,extra):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+extra+[wait,'Enter'],curves=[CURVED,OTHER],measure_jets=True)
    result={}
    for order,h in [(2,3.16),(3,2.26),(4,1.76)]:
        result['control_validate_unequal_g'+str(order)]=pair(order,[dict(click=[10.-h,5.52,0.]),[5.,5.52,0.],dict(click=[2*h,-2.46,0.],shift=True),[2*h+1.,0.,1.]])
    result['control_validate_g3_to_g4']=pair(3,[dict(click=[4.52,-2.46,0.]),[5.2,0.,1.],'Continuity_1=G4','Continuity_2=G4'])
    result['control_validate_numeric_pointer']=pair(2,[dict(click=[6.33,-2.46,0.]),dict(number='2.5'),dict(click=[8.,0.,1.])])
    result['control_validate_point_g4']=case(['Continuity_1=G4',pick(0,[-.25,0.,0.]),'Point',[10.,3.,0.],dict(click=[4.18,-1.1,0.]),[5.,0.,1.],wait,'Enter'],curves=[cubic,line],measure_jets=True)
    result['control_validate_rational_g4']=case(['Continuity_1=G4','Continuity_2=G4',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),dict(click=[2.,-1.,0.]),[3.5,0.,1.],wait,'Enter'],curves=[rational,line],measure_jets=True)
    result['control_validate_join_g3']=case(['Continuity_1=G3','Continuity_2=G3',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),dict(click=[2.64,-.45,0.]),[3.6,0.,1.],'Trim=Yes','Join=Yes',wait,'Enter'],curves=[cubic,line],measure_jets=True)
    return result

def numeric(case):
    wait=dict(wait_prompt='Select control point to adjust')
    def pair(order,extra):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+extra+[wait,'Enter'],curves=[CURVED,OTHER],measure_jets=True)
    result={}
    for order,k,x in [(3,3,6.78),(4,2,3.52),(4,3,5.28),(4,4,7.03)]:
        result['control_number_g'+str(order)+'_k'+str(k)]=pair(order,[dict(click=[x,-2.46,0.]),dict(number='2.5'),[10.,0.,1.]])
    result['control_number_g4_k4_back']=pair(4,[dict(click=[7.03,-2.46,0.]),dict(number='2.5'),[-1.,0.,1.]])
    result['control_number_g2_changed']=pair(2,[dict(click=[6.33,-2.46,0.]),[7.,0.,1.],dict(click=[7.,-2.46,0.]),dict(number='2.5'),[10.,0.,1.]])
    result['control_number_g4_changed']=pair(4,[dict(click=[3.52,-2.46,0.]),[4.1,0.,1.],dict(click=[5.86,-2.46,0.]),dict(number='2.5'),[10.,0.,1.]])
    result['control_number_g4_end']=pair(4,[dict(click=[2.97,5.52,0.]),dict(number='2.5'),[0.,5.,1.]])
    return result

def symmetry(case):
    wait=dict(wait_prompt='Select control point to adjust')
    def pair(order,extra):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+extra+[wait,'Enter'],curves=[CURVED,OTHER],measure_jets=True)
    return dict(
      control_symmetry_third_copies_other_gaps=pair(3,[dict(click=[5.48,5.52,0.]),[4.,5.,1.],dict(click=[6.78,-2.46,0.],shift=True),[8.,0.,1.]]),
      control_symmetry_primary_copies_gaps=pair(3,[dict(click=[4.52,-2.46,0.]),[5.5,0.,1.],dict(click=[3.22,5.52,0.]),[2.,5.,1.],dict(click=[2.26,-2.46,0.],shift=True),[3.,0.,1.]]),
      control_symmetry_mixed_independent=case(['Continuity_1=G3','Continuity_2=G2','Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0),dict(click=[5.28,-2.46,0.],shift=True),[6.,0.,1.],wait,'Enter'],curves=[CURVED,OTHER],measure_jets=True),
      control_continuity_reset=pair(4,[dict(click=[3.52,-2.46,0.]),[4.1,0.,1.],'Continuity_1=G2','Continuity_2=G3']),
    )
