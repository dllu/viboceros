"""Distinct-open contacts and oversized setbacks, observed with actual history."""
from copy import deepcopy


def pilot(case, configured, pick):
    rows = {}
    for label, distances, join, trim in [
        ('zero_parts', (0.,0.), False, True),
        ('zero_joined', (0.,0.), True, True),
        ('zero_untrimmed', (0.,0.), False, False),
        ('oversized_parts', (9.,1.), False, True),
        ('oversized_joined', (9.,1.), True, True),
        ('oversized_untrimmed', (9.,1.), False, False),
        ('consumed_parts', (4.,1.), False, True),
        ('normal_parts', (.5,1.), False, True),
    ]:
        rows['open_chamfer_'+label] = case(configured(distances=distances,join=join,trim=trim)+
            [pick(0,[2.7,0.,0.]),pick(1,[4.,1.7,0.])], capture_undo=True,pending_redo=True)
    for label, distances, join in [('zero_parts',(0.,0.),False),('zero_joined',(0.,0.),True),
                                   ('parts',(.5,1.),False),('joined',(.5,1.),True)]:
        rows['open_chamfer_crossing_'+label] = case(configured(distances=distances,join=join)+
            [pick(0,[2.,0.,0.]),pick(1,[0.,2.,0.])],
            curves=[dict(type='line',start=[-4.,0.,0.],end=[4.,0.,0.]),
                    dict(type='line',start=[0.,-4.,0.],end=[0.,4.,0.])],capture_undo=True,pending_redo=True)
    rows['open_chamfer_parallel_zero_untrimmed'] = case(configured(distances=(0.,0.),trim=False)+
        [pick(0,[2.7,0.,0.]),pick(1,[2.7,2.,0.])],
        curves=[dict(type='line',start=[0.,0.,0.],end=[4.,0.,0.]),
                dict(type='line',start=[0.,2.,0.],end=[4.,2.,0.])],capture_undo=True,pending_redo=True)
    rows['open_chamfer_extension_zero_parts'] = case(configured(distances=(0.,0.))+
        [pick(0,[2.7,0.,0.]),pick(1,[4.,1.7,0.])],
        curves=[dict(type='line',start=[0.,0.,0.],end=[3.,0.,0.]),
                dict(type='line',start=[4.,1.,0.],end=[4.,4.,0.])],capture_undo=True,pending_redo=True)
    r = deepcopy(rows['open_chamfer_oversized_parts'])
    r['actions'][1:3]=[1.,9.];r['actions'][-2:]=list(reversed(r['actions'][-2:]))
    rows['open_chamfer_reverse_oversized_parts']=r
    return rows


def holdouts(case, configured, pick):
    base=pilot(case,configured,pick);rows={}
    for label,distances in [('second',(.5,9.)),('both',(9.,10.)),('below',(3.999,1.)),('above',(4.001,1.)),('near',(3.9999999,1.))]:
        r=deepcopy(base['open_chamfer_oversized_parts']);r['actions'][1:3]=list(distances)
        rows['open_chamfer_oversized_'+label]=r
    for label,indices in [('first',[0]),('second',[1]),('both',[0,1])]:
        r=deepcopy(rows['open_chamfer_oversized_both'])
        for i in indices:r['curves'][i]['start'],r['curves'][i]['end']=r['curves'][i]['end'],r['curves'][i]['start']
        rows['open_chamfer_reversed_'+label]=r
    r=deepcopy(base['open_chamfer_oversized_parts'])
    r['curves'][0]['end']=[2.,0.,0.];r['curves'][1].update(start=[2.,0.,0.],end=[2.,6.,0.]);r['actions'][1:3]=[5.,1.]
    r['actions'][-2:]=[pick(0,[1.,0.,0.]),pick(1,[2.,1.7,0.])];rows['open_chamfer_scaled_oversized']=r
    r=deepcopy(base['open_chamfer_consumed_parts']);r['actions'][4]='Join=Yes';rows['open_chamfer_consumed_joined']=r
    r=deepcopy(base['open_chamfer_crossing_parts']);r['actions'][1:3]=[6.,1.];rows['open_chamfer_crossing_oversized']=r
    r=deepcopy(base['open_chamfer_oversized_parts'])
    def spatial(p):return [10.+.6*p[0],-2.+p[1],3.+.8*p[0]]
    for c in r['curves']:
        for k in ['start','end']:c[k]=spatial(c[k])
    for a in r['actions'][-2:]:a['pick']=spatial(a['pick'])
    rows['open_chamfer_spatial_oversized']=r
    r=deepcopy(base['open_chamfer_zero_parts']);r['actions'][-1]='Cancel';rows['open_chamfer_cancel_second']=r
    for label,style in [('line','Line'),('smooth','Smooth')]:
        r=deepcopy(base['open_chamfer_zero_parts']);r['curves'][1].update(start=[4.,0.,0.],end=[8.,0.,0.]);r['actions'][-1]=pick(1,[5.,0.,0.]);r['actions'].insert(6,'ExtendOtherCurvesBy='+style)
        rows['open_chamfer_collinear_'+label]=r
    for label,trim in [('trimmed',True),('untrimmed',False)]:
        r=deepcopy(base['open_chamfer_crossing_parts']);r['curves'][1].update(start=[0.,-4.,.5],end=[0.,4.,.5]);r['actions'][5]='Trim='+('Yes'if trim else'No');r['actions'][-1]['pick']=[0.,2.,.5]
        rows['open_chamfer_skew_'+label]=r
    nurbs=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[1.,0.,0.],[2.,1.,0.]]],knots=[0.,0.,0.,1.,1.,1.])
    rows['open_chamfer_zero_nurbs']=case(configured(distances=(0.,0.))+[pick(0,[1.5,.5625,0.]),pick(1,[2.,2.,0.])],curves=[nurbs,dict(type='line',start=[2.,1.,0.],end=[2.,5.,0.])],capture_undo=True,pending_redo=True)
    arc=dict(type='arc',points=[[1.,0.,0.],[2.**-.5,2.**-.5,0.],[0.,1.,0.]])
    for label,style in [('arc','Arc'),('line','Line')]:
        rows['open_chamfer_zero_arc_'+label]=case(configured(distances=(0.,0.))+['ExtendArcsBy='+style,'ExtendOtherCurvesBy=Line',pick(0,[.3,.91**.5,0.]),pick(1,[-.5,2.5,0.])],curves=[arc,dict(type='line',start=[-.5,2.,0.],end=[-.5,3.,0.])],capture_undo=True,pending_redo=True)
    for label,style in [('line','Line'),('smooth','Smooth')]:
        rows['open_chamfer_zero_nurbs_'+label]=case(configured(distances=(0.,0.))+['ExtendOtherCurvesBy='+style,pick(0,[1.5,.5625,0.]),pick(1,[3.,3.,0.])],curves=[nurbs,dict(type='line',start=[3.,2.,0.],end=[3.,4.,0.])],capture_undo=True,pending_redo=True)
    return rows


def curves_and_joins(case, configured, pick):
    base=pilot(case,configured,pick);rows={}
    for label,distances in [('second',(.5,4.)),('both',(4.,4.))]:
        r=deepcopy(base['open_chamfer_zero_joined']);r['actions'][1:3]=list(distances)
        rows['open_chamfer_consumed_joined_'+label]=r
    for label,join,distance in [('consumed_parts',False,4.),('consumed_joined',True,4.),('oversized_joined',True,6.)]:
        r=deepcopy(base['open_chamfer_crossing_parts']);r['actions'][1:3]=[distance,1.];r['actions'][4]='Join='+('Yes'if join else'No')
        rows['open_chamfer_crossing_'+label]=r
    for style in ['Line','Smooth']:
        for label,join,trim in [('parts',False,True),('joined',True,True),('untrimmed',False,False)]:
            r=deepcopy(holdouts(case,configured,pick)['open_chamfer_collinear_'+style.lower()]);r['actions'][1:3]=[.5,1.];r['actions'][4]='Join='+('Yes'if join else'No');r['actions'][5]='Trim='+('Yes'if trim else'No')
            rows['open_chamfer_collinear_'+style.lower()+'_'+label]=r
    r=deepcopy(holdouts(case,configured,pick)['open_chamfer_collinear_smooth']);r['actions'][4]='Join=Yes';rows['open_chamfer_collinear_smooth_zero_joined']=r
    for style in ['Line','Smooth']:
        for join in [False,True]:
            r=deepcopy(holdouts(case,configured,pick)['open_chamfer_zero_nurbs']);r['actions'][1:3]=[3.,1.];r['actions'][4]='Join='+('Yes'if join else'No');r['actions'].insert(6,'ExtendOtherCurvesBy='+style)
            rows['open_chamfer_nurbs_consumed_'+style.lower()+('_joined'if join else'_parts')]=r
    arc=dict(type='arc',points=[[2.,0.,0.],[2.**.5,2.**.5,0.],[0.,2.,0.]])
    for label,distance,join in [('normal',.5,False),('beyond',3.,False),('beyond_joined',3.,True),('outside',5.,False)]:
        rows['open_chamfer_arc_'+label]=case(configured(distances=(distance,1.),join=join)+['ExtendArcsBy=Arc','ExtendOtherCurvesBy=Line',pick(0,[.6,2.*.91**.5,0.]),pick(1,[0.,3.5,0.])],curves=[arc,dict(type='line',start=[0.,2.,0.],end=[0.,6.,0.])],capture_undo=True,pending_redo=True)
    for label,curve,line,p0,p1 in [
        ('nurbs',holdouts(case,configured,pick)['open_chamfer_zero_nurbs']['curves'][0],dict(type='line',start=[1.,-2.,0.],end=[1.,2.,0.]),[1.5,.5625,0.],[1.,1.5,0.]),
        ('arc',dict(type='arc',points=[[1.,0.,0.],[2.**-.5,2.**-.5,0.],[0.,1.,0.]]),dict(type='line',start=[.5,0.,0.],end=[.5,2.,0.]),[.3,.91**.5,0.],[.5,1.7,0.]),
    ]:
        for join in [False,True]:
            rows['open_chamfer_crossing_'+label+('_joined'if join else'_parts')]=case(configured(distances=(.1,.2),join=join)+[pick(0,p0),pick(1,p1)],curves=[curve,line],capture_undo=True,pending_redo=True)
    return rows


def extension_choices(case, configured, pick):
    base=curves_and_joins(case,configured,pick);rows={}
    for label,a,b,p0,p1 in [('short',2.,2.,1.5,2.5),('unequal',4.,6.,3.,5.),('upper',4.,4.,3.,7.)]:
        r=deepcopy(base['open_chamfer_collinear_smooth_parts'])
        r['curves'][0]['end']=[a,0.,0.];r['curves'][1].update(start=[a,0.,0.],end=[a+b,0.,0.]);r['actions'][-2:]=[pick(0,[p0,0.,0.]),pick(1,[p1,0.,0.])]
        rows['open_chamfer_smooth_'+label]=r
    r=deepcopy(base['open_chamfer_arc_beyond']);r['curves'][0]['points'].reverse();rows['open_chamfer_arc_beyond_reversed']=r
    r=deepcopy(base['open_chamfer_nurbs_consumed_line_parts']);r['curves'][0]['control_points'].reverse();rows['open_chamfer_nurbs_consumed_reversed']=r
    r=deepcopy(base['open_chamfer_nurbs_consumed_line_parts']);r['actions'][5]='Trim=No';rows['open_chamfer_nurbs_consumed_untrimmed']=r
    return rows


def overlap_choices(case, configured, pick):
    base=curves_and_joins(case,configured,pick)['open_chamfer_collinear_smooth_parts'];rows={}
    for label,end,p0,p1 in [('overlap',6.,3.5,7.),('separated',3.,2.,4.7)]:
        r=deepcopy(base);r['curves'][0]['end']=[end,0.,0.];r['actions'][-2:]=[pick(0,[p0,0.,0.]),pick(1,[p1,0.,0.])]
        rows['open_chamfer_smooth_'+label]=r
    return rows
