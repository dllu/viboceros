"""Owned native SelChain seeds, with no external geometry or scripts."""
def discovery_cases():
    rows=[]
    for name,scene,seed,pre,finish in [
        ('empty','lines',None,[],'Cancel'),('preselected','lines',None,[1],'Cancel'),
        ('lines_accept','lines',1,[],'Enter'),('lines_cancel','lines',1,[],'Cancel'),
        ('corner','corner',0,[],'Enter'),('surface','surface',0,[],'Enter'),
        ('mixed_curve','mixed',1,[],'Enter'),('mixed_edge','mixed',0,[],'Enter')]:
        rows.append(dict(op='chain_selection',id=name,scene=scene,seed=seed,pre=pre,steps=[] if seed is None else [dict(kind='click',modifiers='plain')],finish=finish))
    return rows

def source(scene):
    def line(a,b):return dict(kind='line',points=[a,b])
    def p(x,y):return [float(x),float(y),0.]
    if scene.startswith('arc_') or scene=='line_arc' or scene=='closed_circle':
        import math
        q=math.sqrt(.5)
        if scene=='closed_circle':return [dict(kind='circle',points=[p(-3,0),p(0,3),p(3,0)]),line(p(3,0),p(6,0))]
        r=1. if scene in ('arc_opposite','line_arc') else float(scene.split('_')[-1])
        sign=-1. if scene=='arc_opposite' else 1.
        first=line(p(-3,0),p(0,0)) if scene=='line_arc' else dict(kind='arc',points=[p(-1,1),p(-q,1-q),p(0,0)])
        rows=[first,dict(kind='arc',points=[p(0,0),p(r*q,sign*r*(1-q)),p(r,sign*r)])]
        if scene.startswith('arc_scaled_'):
            scale=float(scene.split('_')[-2])
            for row in rows:row['points']=[[x*scale,y*scale,z*scale]for x,y,z in row['points']]
        if scene.startswith('arc_spatial_'):
            for row in rows:row['points']=[[x,y*q,y*q]for x,y,z in row['points']]
        return rows
    if scene=='lines':return [line(p(i*3,0),p((i+1)*3,0)) for i in range(3)]+[line(p(0,4),p(3,4))]
    if scene=='reversed':return [line(p(3,0),p(0,0)),line(p(3,0),p(6,0)),line(p(9,0),p(6,0))]
    if scene=='cycle':return [line(p(0,0),p(3,0)),line(p(3,0),p(3,3)),line(p(3,3),p(0,3)),line(p(0,3),p(0,0))]
    if scene=='branch':return [line(p(0,0),p(3,0)),line(p(3,0),p(6,0)),line(p(3,0),p(6,3))]
    if scene=='subdoc_gap':return [line(p(0,0),p(3,0)),line(p(3.0000005,0),p(6,0))]
    if scene=='gap':return [line(p(0,0),p(3,0)),line(p(3.00001,0),p(6,0))]
    if scene.startswith('angle_'):
        import math
        angle=float(scene.split('_')[1])*math.pi/180.
        return [line(p(0,0),p(3,0)),line(p(3,0),p(3+3*math.cos(angle),3*math.sin(angle)))]
    if scene=='prefer_curve':return [line(p(0,0),p(3,0)),line(p(3,0),p(6,0)),dict(kind='surface',points=[p(3,0),p(6,0),p(6,3),p(3,3)])]
    if scene=='corner':return [line(p(0,0),p(3,0)),line(p(3,0),p(3,3)),line(p(3,3),p(6,3))]
    if scene in ('surface','mixed'):
        rows=[dict(kind='surface',points=[p(0,0),p(3,0),p(3,3),p(0,3)])]
        if scene=='mixed':rows.append(line(p(3,0),p(6,0)))
        return rows
    raise ValueError('owned SelChain source required')

def validate(op):
    if not isinstance(op,dict) or op not in discovery_cases()+cases()+curvature_cases()+curvature_limit_cases()+curvature_boundary_cases()+acceptance_cases()+fork_cases()+angle_precision_cases()+option_minimum_cases()+option_boundary_cases()+ordering_cases() or any(type(i) is not int for i in op.get('pre',[])) or any('source' in step and type(step['source']) is not int for step in op.get('steps',[])) or ('inspect' in op and type(op['inspect']) is not bool) or (op.get('seed') is not None and type(op['seed']) is not int):
        raise ValueError('fixed owned SelChain operation required')


def cases():
    rows=[]
    def add(name,scene,seed=0,steps=None,finish='Enter',pre=None,**options):
        row=dict(op='chain_selection',id=name,scene=scene,seed=seed,pre=pre or [],steps=steps or [dict(kind='click',modifiers='plain')],finish=finish,options=options)
        if scene in ('branch','prefer_curve') and options.get('continuity')=='Position' and options.get('direction')!='Backward':row['steps']=row['steps']+[dict(kind='key',value='Enter')]
        rows.append(row)
    for scene in ('lines','reversed','corner','cycle','surface','mixed','prefer_curve','branch'):
        for continuity in ('Position','Tangency','Curvature'):
            for direction in ('Both','Forward','Backward'):
                add(scene+'_'+continuity+'_'+direction,scene,1 if scene in ('lines','reversed') else 0,continuity=continuity,direction=direction)
    for gap in ('0','0.000001','0.00001','0.0001'):
        add('gap_'+gap.replace('.','_'),'gap',gap=gap)
    for angle in ('0.999','1','1.001'):
        add('angle_'+angle.replace('.','_'),'angle_'+angle,angle='1')
    click=dict(kind='click',modifiers='plain')
    for name,scene,steps,finish,flags in [
        ('undo_lines','lines',[click,dict(kind='key',value='Undo')],'Enter',{}),
        ('next_corner','corner',[click,dict(kind='key',value='Next')],'Enter',{}),
        ('all_corner','corner',[click,dict(kind='key',value='All')],'Enter',{}),
        ('position_after_pick','corner',[click,dict(kind='key',value='ChainContinuity=Position'),dict(kind='key',value='All')],'Enter',{}),
        ('cancel_preselected','lines',[click],'Cancel',dict(pre=[3])),
        ('accept_preselected','lines',[click],'Enter',dict(pre=[3])),
        ('pick_preselected_seed','lines',[click],'Enter',dict(pre=[0])),
        ('repeat_seed','lines',[click,click],'Enter',{}),
        ('ctrl_seed','lines',[click,dict(kind='click',modifiers='ctrl')],'Enter',{})]:
        add(name,scene,steps=steps,finish=finish,**flags)
    return rows

def curvature_cases():
    return [dict(op='chain_selection',id=scene+'_'+continuity,scene=scene,seed=0,pre=[],steps=[dict(kind='click',modifiers='plain')],finish='Enter',options=dict(continuity=continuity)) for scene in ['arc_0.5','arc_1.0','arc_1.04','arc_1.06','arc_2.0','arc_opposite','arc_spatial_1.04','line_arc','closed_circle'] for continuity in ('Tangency','Curvature')]

def curvature_limit_cases():
    rows=[]
    for scene in ['arc_1.0000000001','arc_1.00000001','arc_1.0000001','arc_1.000001','arc_1.00001','arc_scaled_1000000_1.04','arc_scaled_10000000_1.04','arc_scaled_0.0001_1.000000001']:
        rows.append(dict(op='chain_selection',id=scene.replace('.','_'),scene=scene,seed=0,pre=[],steps=[dict(kind='click',modifiers='plain')],finish='Enter',options=dict(continuity='Curvature')))
    rows.append(dict(rows[2],id='curvature_larger_gap',options=dict(continuity='Curvature',gap='0.0001')))
    return rows

def curvature_boundary_cases():
    return [dict(op='chain_selection',id='curvature_boundary_'+str(i),scene=scene,seed=0,pre=[],steps=[dict(kind='click',modifiers='plain')],finish='Enter',options=dict(continuity='Curvature')) for i,scene in enumerate(['arc_1.000000011','arc_1.0000000148','arc_1.000000015'])]

def acceptance_cases():
    return [dict(op='chain_selection',id='accept_'+scene,scene=scene,seed=0,pre=[],steps=[dict(kind='click',modifiers='plain'),dict(kind='key',value='Enter')],finish='Enter',options=dict(continuity='Position'),inspect=True) for scene in ('branch','prefer_curve')]

def fork_cases():
    rows=[]
    for action in ('Next','All','Undo'):
        rows.append(dict(op='chain_selection',id='fork_'+action,scene='branch',seed=0,pre=[],steps=[dict(kind='click',modifiers='plain'),dict(kind='key',value=action),dict(kind='key',value='Enter')],finish='Enter',options=dict(continuity='Position')))
    for target in (1,2):
        rows.append(dict(op='chain_selection',id='fork_pick_'+str(target),scene='branch',seed=0,pre=[],steps=[dict(kind='click',modifiers='plain'),dict(kind='click',modifiers='plain',source=target),dict(kind='key',value='Enter')],finish='Enter',options=dict(continuity='Position')))
    return rows

def angle_precision_cases():
    return [dict(op='chain_selection',id='angle_precision_'+str(i),scene=scene,seed=0,pre=[],steps=[dict(kind='click',modifiers='plain')],finish='Enter',options=dict(angle=angle)) for i,(scene,angle) in enumerate([('lines','0'),('lines','0.000000001'),('angle_0.000000001','0.000000002')])]

def option_minimum_cases():
    return [dict(op='chain_selection',id='option_minimum_'+str(i),scene=scene,seed=0,pre=[],steps=[dict(kind='click',modifiers='plain')],finish='Enter',options=options) for i,(scene,options) in enumerate([('subdoc_gap',dict(gap='0')),('subdoc_gap',dict(gap='0.00000001')),('angle_0.00004',dict(angle='0.00002')),('angle_0.00004',dict(angle='0'))])]


def option_boundary_cases():
    return [dict(op='chain_selection',id='option_boundary_'+str(i),scene=scene,seed=0,pre=[],steps=[dict(kind='click',modifiers='plain')],finish='Enter',options=options) for i,(scene,options) in enumerate([('subdoc_gap',dict(gap='0.0000000001')),('subdoc_gap',dict(gap='0.0000000003')),('angle_0.00004',dict(angle='0.00000001')),('angle_0.00004',dict(angle='0.00000002'))])]


def ordering_cases():
    return [dict(op='chain_selection',id='middle_undo_'+str(n),scene='lines',seed=1,pre=[],steps=[dict(kind='click',modifiers='plain')]+[dict(kind='key',value='Undo')]*n,finish='Enter',options=dict(direction='Both')) for n in (1,2)]
