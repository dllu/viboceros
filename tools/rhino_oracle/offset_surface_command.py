"""Prescribed public OffsetSrf inputs, independent of observed output geometry."""
PLANE = dict(type='normal_surface',target=dict(kind='brep',points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]],aim=[2.,1.,0.],view='Top',flip=False))

def case(actions, **kwargs):
    value=dict(curves=[PLANE],view='Top',preselected=True,actions=actions,pending_redo=True,capture_undo=True)
    value.update(kwargs)
    return value

PILOT = {'offset_prompt_cancel':case(['Cancel']), 'offset_default_accept':case(['Enter'])}
BASE = ['Corner=Sharp','Solid=No','BothSides=No','DeleteInput=No','Distance=1']
STAGES = {
    'offset_number_cancel':case(BASE+[2.25,'Cancel']),
    'offset_menu_accept':case(BASE+['Distance',1.25,'Enter']),
    'offset_plain_accept':case(BASE+['Enter']),
    'offset_postselect_accept':case([{'curve':0},'Enter']+BASE+['Enter'],preselected=False),
}
BASELINE = ['Corner=Sharp','Solid=Yes','DeleteInput=No','Solid=No','BothSides=No','Distance',0.5]
QUALIFIED = {
 'offset_q_plain':case(BASELINE+['Enter']),
 'offset_q_negative':case(BASELINE+[-0.75,'Enter']),
 'offset_q_solid_keep':case(BASELINE+['Solid=Yes','Enter']),
 'offset_q_solid_delete':case(BASELINE+['Solid=Yes','DeleteInput=Yes','Enter']),
 'offset_q_both':case(BASELINE+['BothSides=Yes','Enter']),
 'offset_q_both_solid':case(BASELINE+['BothSides=Yes','Solid=Yes','Enter']),
 'offset_q_flip':case(BASELINE+['FlipAll','Enter']),
 'offset_q_click_flip':case(BASELINE+[{'click':[2.,1.,0.]},'Enter'],shaded=True),
 'offset_q_number_cancel':case(BASELINE+[2.25,'Cancel']),
 'offset_q_zero_cancel':case(BASELINE+[0.,'Cancel']),
 'offset_q_distance_cancel':case(BASELINE+['Distance','Cancel','Cancel']),
 'offset_q_postselect':case([{'curve':0},'Enter']+BASELINE+['Enter'],preselected=False),
 'offset_q_postselect_cancel':case([{'curve':0},'Enter']+BASELINE+['Cancel'],preselected=False),
 'offset_q_postselect_delete':case([{'curve':0},'Enter']+BASELINE+['Solid=Yes','DeleteInput=Yes','Enter'],preselected=False),
 'offset_q_through':case(BASELINE+['ThroughPoint',[2.,1.,0.75],'Enter']),
}
def target_case(actions, **changes):
    import copy
    plane=copy.deepcopy(PLANE)
    plane['target'].update(changes)
    return case(BASELINE+actions,curves=[plane])

HOLDOUTS = {
 'offset_h_zero':case(BASELINE+[0.,'Enter']),
 'offset_h_zero_solid':case(BASELINE+['Solid=Yes',0.,'Enter']),
 'offset_h_tiny':case(BASELINE+[1e-7,'Enter']),
 'offset_h_negative_both':case(BASELINE+[-0.75,'BothSides=Yes','Enter']),
 'offset_h_double_flip':case(BASELINE+['FlipAll','FlipAll','Enter']),
 'offset_h_reversed':target_case(['Enter'],flip=True),
 'offset_h_tilted':target_case(['Enter'],points=[[1.,2.,3.],[5.,2.,3.],[5.,4.4,4.8],[1.,4.4,4.8]]),
 'offset_h_trimmed':target_case(['Enter'],kind='trimmed'),
 'offset_h_two':case(BASELINE+['Enter'],curves=[PLANE,dict(type='normal_surface',target=dict(kind='brep',points=[[6.,0.,1.],[10.,0.,1.],[10.,3.,1.],[6.,3.,1.]],aim=[8.,1.,1.],view='Top',flip=True))]),
 'offset_h_units':case(BASELINE+[{'number':'.125cm'},'Enter']),
 'offset_h_distance_points':case(BASELINE+['Distance',[0.,0.,0.],[0.,0.,2.],'Enter']),
 'offset_h_through':case(BASELINE+['ThroughPoint',[2.,1.,0.75]]),
 'offset_h_through_flip':case(BASELINE+['FlipAll','ThroughPoint',[2.,1.,0.75]]),
 'offset_h_through_outside':case(BASELINE+['ThroughPoint',[6.,1.,0.75]]),
 'offset_h_through_two':case(BASELINE+['ThroughPoint',[2.,1.,0.75]],curves=[PLANE,dict(type='normal_surface',target=dict(kind='brep',points=[[6.,0.,1.],[10.,0.,1.],[10.,3.,1.],[6.,3.,1.]],aim=[8.,1.,1.],view='Top',flip=True))]),
 'offset_h_through_trimmed':target_case(['ThroughPoint',[3.,1.,0.75]],kind='trimmed'),
}
FINAL = {
 'offset_f_distance_points':case(BASELINE+['Distance',[0.,0.,0.],[0.,0.,2.],'Enter']),
 'offset_f_zero_solid_delete':case(BASELINE+['Solid=Yes','DeleteInput=Yes',0.,'Enter']),
 'offset_f_zero_both_solid':case(BASELINE+['Solid=Yes','BothSides=Yes',0.,'Enter']),
 'offset_f_through_below':case(BASELINE+['ThroughPoint',[2.,1.,-0.75]]),
 'offset_f_through_both':case(BASELINE+['BothSides=Yes','ThroughPoint',[2.,1.,0.75]]),
 'offset_f_through_solid':case(BASELINE+['Solid=Yes','ThroughPoint',[2.,1.,0.75]]),
 'offset_f_through_cancel':case(BASELINE+['ThroughPoint','Cancel']),
 'offset_f_cancel_selection':case(['Cancel'],preselected=False),
}
COMPLETION = {
 'offset_c_through_cancel':case(BASELINE+['ThroughPoint','Cancel','Cancel']),
 'offset_c_zero_both_delete':case(BASELINE+['Solid=Yes','BothSides=Yes','DeleteInput=Yes',0.,'Enter']),
 'offset_c_distance_keep':case(BASELINE+['Distance','Enter','Enter']),
}
BATCH = {
 'offset_b_through_near_second':case(BASELINE+['ThroughPoint',[8.,1.,0.75]],curves=HOLDOUTS['offset_h_two']['curves']),
 'offset_b_through_reverse_picks':case([{'curve':1},{'curve':0},'Enter']+BASELINE+['ThroughPoint',[8.,1.,0.75]],curves=HOLDOUTS['offset_h_two']['curves'],preselected=False),
 'offset_b_zero_distance_default':case(BASELINE+[0.,'Distance','Enter','Enter']),
 'offset_b_negative_distance_default':case(BASELINE+[-0.75,'Distance','Enter','Enter']),
}
MENUS = {
 'offset_m_negative_default':case(BASELINE+[-0.75,'Distance','Enter',0.5,'Enter']),
 'offset_m_negative_cancel':case(BASELINE+[-0.75,'Distance','Enter','Cancel','Cancel']),
 'offset_m_negative_number':case(BASELINE+['Distance',-0.75,0.25,'Enter']),
 'offset_m_zero_number':case(BASELINE+['Distance',0.,0.25,'Enter']),
}
CASES = dict(PILOT)
CASES.update(STAGES)
CASES.update(QUALIFIED)
CASES.update(HOLDOUTS)
CASES.update(FINAL)
CASES.update(COMPLETION)
CASES.update(BATCH)
CASES.update(MENUS)

def validate(op):
    if not isinstance(op,dict) or set(op) != {'op','id','case'} or op.get('op') != 'offset_surface_command' or op.get('id') != op.get('case') or op.get('case') not in CASES:
        raise ValueError('invalid prescribed OffsetSrf getter recipe')

def request(names):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='offset_surface_command',id=name,case=name)for name in names])
