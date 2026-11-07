"""Prescribed physical Alt selections and point targets on supporting faces."""
if __package__:from .blend_curve_edge_cases import PLANE,OTHER,CURVED,SKEW,edge
else:from blend_curve_edge_cases import PLANE,OTHER,CURVED,SKEW,edge
def pilot(case):
    def pair(extra,order=2,curves=None):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+extra+['Enter'],curves=curves or[PLANE,OTHER],measure_jets=True)
    def click(p,alt=False,shift=False):return dict(click=p,alt=alt,shift=shift)
    p=[3.16,-2.46,0.];q=[4.,0.,0.]
    return dict(
        alt_baseline=pair([click(p),click(q)]),
        alt_select_typed=pair([click(p,True),q]),
        alt_select_pointer=pair([click(p,True),click(q)]),
        alt_target_pointer=pair([click(p),click(q,True)]),
        alt_both_pointer=pair([click(p,True),click(q,True)]),
        alt_second_pointer=pair([click([6.83,5.52,0.],True),click([6.,3.,0.],True)]),
        alt_shift_pointer=pair([click(p,True,True),click(q,True)]),
        alt_curved_g3=pair([click([2.26,-2.5,0.],True),click(q,True)],3,[CURVED,OTHER]),
    )

def verification(case,pick,line):
    result={'verified_'+name:value for name,value in pilot(case).items()}
    def click(p,alt=False,shift=False):return dict(click=p,alt=alt,shift=shift)
    def pair(extra,order=2,curves=None,options=None):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0)]+(options or[])+extra+['Enter'],curves=curves or[PLANE,OTHER],measure_jets=True)
    p=[3.16,-2.46,0.];q=[4.,0.,0.]
    for order in [1,2,3,4]:
        shape={1:5.28,2:3.16,3:2.26,4:1.76}[order]
        result['alt_order_g'+str(order)]=pair([click([shape,-2.46,0.],True),q],order,[CURVED,OTHER])
    result.update(
        alt_off_plane=pair([click(p,True),[4.,0.,7.]]),
        alt_backwards=pair([click(p,True),[-4.,0.,0.]]),
        alt_along_edge=pair([click(p,True),[0.,1.,0.]]),
        alt_negative_angle=pair([click(p,True),[4.,-5.,0.]]),
        alt_preexisting_angle=pair([click([2.74,-.88,0.],True),q],options=['AtAngle_1=30']),
        alt_after_length=pair([click(p),[4.,-2.5,0.],click([4.,-2.46,0.],True),q]),
        alt_then_length=pair([click(p,True),q,click([2.7,-.8,0.]),[4.,1.,0.]]),
        alt_shift_unequal=pair([click([6.83,5.52,0.]),[5.,5.5,0.],click(p,True,True),q]),
        alt_numeric_angle=pair([click(p,True),dict(number='30')]),
        alt_curved_skew=pair([click(p,True),[4.,0.,3.]],curves=[SKEW,OTHER]),
        alt_ordinary=case([pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),click([1.8,0.,0.],True),[3.,2.,1.],'Enter'],measure_jets=True),
        alt_mixed_join=case(['Edges',edge(0,[0.,-2.5,0.],2),'Curves',pick(1,[10.25,3.,0.]),click([2.8,-2.46,0.],True),q,'Trim=Yes','Join=Yes','Enter'],curves=[PLANE,line],measure_jets=True),
    )
    cancelled=pair([click(p,True),q]);cancelled['actions'][-1]='Cancel';cancelled['pending_redo']=True;result['alt_cancel']=cancelled
    return result

def followups(case,pick,line):
    import copy,math
    result={};base=verification(case,pick,line)
    fixed=copy.deepcopy(base['alt_preexisting_angle']);fixed['actions'][6]['click']=[2.74,-4.04,0.];result['alt_existing_angle_corrected']=fixed
    numeric=copy.deepcopy(base['alt_numeric_angle']);numeric['actions'][6]['number']='-35';result['alt_numeric_negative']=numeric
    second=copy.deepcopy(base['verified_alt_second_pointer']);second['actions'][6]=dict(number='25');result['alt_numeric_second']=second
    for order in [2,4]:
        spatial=copy.deepcopy(base['alt_order_g'+str(order)]);spatial['view']='Perspective'
        def rotate(p):return [.8*p[0]+.6*p[2]+2.,p[1]+1.,-.6*p[0]+.8*p[2]-3.]
        for source in spatial['curves']:
            for p in source['control_points']:p['point']=rotate(p['point'])
        for action in spatial['actions']:
            if isinstance(action,dict):
                for key in ['pick','click']:
                    if key in action:action[key]=rotate(action[key])
            elif isinstance(action,list):action[:]=rotate(action)
        result['alt_spatial_g'+str(order)]=spatial
    skew=copy.deepcopy(base['alt_curved_skew']);skew['actions'][0]='Continuity_1=G4';skew['actions'][1]='Continuity_2=G4';skew['actions'][5]['click']=[1.8,-2.46,0.];result['alt_skew_g4']=skew
    result['alt_surface_point']=case(['Edges',edge(0,[0.,-2.5,0.],2),'Point',[10.,3.,2.],dict(click=[math.sqrt(134.25)/3.,-2.46,0.],alt=True),[4.,0.,3.],'Enter'],curves=[PLANE,line],measure_jets=True)
    result['alt_mixed_reverse']=case([pick(1,[10.25,3.,0.]),'Edges',edge(0,[0.,-2.5,0.],2),dict(click=[2.8,-2.46,0.],alt=True),[4.,0.,0.],'Enter'],curves=[PLANE,line],measure_jets=True)
    contact=copy.deepcopy(base['verified_alt_select_typed']);contact['actions'][5]['click']=[0.,-2.46,0.];contact['actions'][6]=[0.,-1.,0.];result['alt_contact']=contact
    return result

def picking(case,pick):
    return dict(
        alt_far_surface=case(['Edges',edge(0,[0.,-2.5,0.],2),edge(1,[10.,5.5,0.],0),dict(click=[0.,10.,0.],alt=True),[6.,3.,0.],'Enter'],curves=[PLANE,OTHER],measure_jets=True),
        alt_far_ordinary=case([pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),dict(click=[0.,8.,0.],alt=True),[-1.,0.,0.],'Enter'],measure_jets=True),
    )

def plain_selection(case,pick):
    import copy
    result={}
    for name,value in picking(case,pick).items():
        value=copy.deepcopy(value)
        for action in value['actions']:
            if isinstance(action,dict)and 'alt'in action:action['alt']=False
        result['plain_'+name]=value
    return result
