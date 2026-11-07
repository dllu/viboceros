"""Prescribed DisplayCurvature input; no measured output controls seed edits."""
def pilot(case,pick):
    return {'display_prompt':case(['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No','DisplayCurvature',dict(wait_prompt=''),'Enter','Enter'],measure_jets=True)}

def discovery(case,pick,arc,line):
    actions=['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No',
             dict(view_snapshot='initial'),'DisplayCurvature',dict(view_snapshot='once'),
             'DisplayCurvature',dict(view_snapshot='twice'),'Enter']
    return {'display_arc_toggles':case(actions,curves=[arc,line],measure_jets=True),
            'display_prompt_repeat':case(['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No','DisplayCurvature',dict(wait_prompt=''),'Enter'],measure_jets=True)}

def lifecycle(case,pick,arc,line):
    prefix=['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No']
    def make(extra,**kwargs):
        return case(prefix+extra,curves=[arc,line],measure_jets=True,measure_curvature=True,display_completion=True,**kwargs)
    return {
        'display_enable_lifetime':make([dict(view_snapshot='initial'),'DisplayCurvature',dict(view_snapshot='desktop_once'),'DisplayCurvature',dict(view_snapshot='twice'),'Enter'],desktop_after=True,graph_settings=dict(scale=110,density=3)),
        'display_reentry':make([dict(view_snapshot='reentry'),'Enter'],graph_settings=dict(scale=110,density=3)),
        'display_cancel_lifetime':make(['DisplayCurvature',dict(view_snapshot='before_cancel'),'Cancel'],pending_redo=True,graph_settings=dict(scale=110,density=3)),
    }

def holdouts(case,pick,arc,line,cubic,cubic2,surface):
    import copy
    prefix=['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No']
    def make(actions=None,curves=None,**kwargs):
        return case(actions or prefix+[dict(view_snapshot='initial'),'DisplayCurvature',dict(view_snapshot='enabled'),'Enter'],curves=curves or[arc,line],measure_jets=True,measure_curvature=True,display_completion=True,graph_settings=dict(scale=106,density=4),**kwargs)
    result={
        'display_existing_selected':make(initial_graphs=[0]),
        'display_existing_unrelated':make(curves=[arc,line,cubic2],initial_graphs=[2]),
        'display_point_endpoint':make(['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),'Point',[10.,3.,2.],'Trim=No','Join=No','DisplayCurvature',dict(view_snapshot='enabled'),'Enter']),
        'display_portion':make(['BlendStart=CurveEnd','SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],[-1.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Join=No','DisplayCurvature',dict(view_snapshot='enabled'),'Enter'],curves=[dict(type='line',start=[-5.,0.,0.],end=[0.,0.,0.]),line]),
        'display_order_change':make(['BlendStart=CurveEnd','Continuity_1=G4','Continuity_2=G4',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No','DisplayCurvature',dict(view_snapshot='g4'),'Continuity_2=G1',dict(view_snapshot='changed'),'Enter'],curves=[cubic,line]),
        'display_transparent_off':make(prefix+['DisplayCurvature',dict(view_snapshot='before_off'),dict(transparent='CurvatureGraphOff'),dict(view_snapshot='after_off'),'Enter']),
    }
    circle=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
    result['display_closed']=make(['BlendStart=CurveEnd',pick(0,[0.,5.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No','DisplayCurvature',dict(view_snapshot='enabled'),'Enter'],curves=[circle,line])
    recipe=copy.deepcopy(surface);recipe['actions']=recipe['actions'][:-1]+['DisplayCurvature',dict(view_snapshot='enabled'),'Enter'];recipe.update(measure_curvature=True,display_completion=True,graph_settings=dict(scale=106,density=4));result['display_surface']=recipe
    return result

def verification(case,pick,previous,line):
    import copy
    result={}
    names=['display_existing_unrelated']+[name for name in previous if name!='display_existing_unrelated']
    for name in names:
        recipe=copy.deepcopy(previous[name]);recipe['reset_graphs']=True
        if name=='display_existing_unrelated':
            for control in recipe['curves'][2]['control_points']:control['point'][0]+=20.
        result['verified_'+name]=recipe
    for cancel in [False,True]:
        recipe=copy.deepcopy(result['verified_display_existing_selected']);recipe['actions']=[a for a in recipe['actions']if a!='DisplayCurvature']
        if cancel:recipe['actions'][-1]='Cancel';recipe['pending_redo']=True
        result['verified_existing_without_display'+('_cancel'if cancel else'')]=recipe
    circle=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
    result['verified_display_curved_portion']=case(['BlendStart=CurveEnd','SubCrv',pick(0,[5.,0.,0.]),[5.,0.,0.],[0.,5.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Join=No','DisplayCurvature',dict(view_snapshot='enabled'),'Enter'],curves=[circle,line],measure_jets=True,measure_curvature=True,display_completion=True,graph_settings=dict(scale=106,density=4),reset_graphs=True)
    return result

def defaults(case,pick,arc,line):
    sources=[pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.])]
    def make(extra,picks=None,**kwargs):
        return case(['BlendStart=CurveEnd']+extra+(sources if picks is None else picks)+['Trim=No','Join=No',dict(view_snapshot='ready')]+(['Cancel']if kwargs.get('pending_redo')else['Enter']),curves=[arc,line],measure_jets=True,measure_curvature=True,display_completion=True,reset_graphs=True,**kwargs)
    return {
        'display_defaults_low':make(['Continuity_1=G0','Continuity_2=G1']),
        'display_defaults_low_next':make([]),
        'display_defaults_high_cancel':make(['Continuity_1=G3','Continuity_2=G4'],pending_redo=True),
        'display_defaults_high_next':make([]),
        'display_defaults_point':make([],picks=[pick(0,[-.25,0.,0.]),'Point',[10.,3.,2.]]),
        'display_defaults_after_point':make([]),
    }

def control_defaults(case,pick,arc,line):
    prefix=['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Trim=No','Join=No']
    def make(options,cancel=False):
        return case(prefix+options+[dict(view_snapshot='ready'),'Cancel'if cancel else'Enter'],curves=[arc,line],measure_jets=True,measure_curvature=True,display_completion=True,reset_graphs=True,**({'pending_redo':True}if cancel else{}))
    return {
        'display_control_defaults_low':make(['Continuity_1=G0','Continuity_2=G1']),
        'display_control_defaults_low_next':make([]),
        'display_control_defaults_high_cancel':make(['Continuity_1=G3','Continuity_2=G4'],True),
        'display_control_defaults_high_next':make([]),
    }
