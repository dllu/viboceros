"""Endpoint flips that leave no retained source interval after trimming."""
def discovery(case,pick):
    result={}
    for name,flips in [('first',[0]),('second',[1]),('both',[0,1])]:
        for join in [False,True]:
            actions=['BlendStart=CurveEnd',pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.])]
            for i in flips:actions+=['Flip',pick(i,[-.25,0.,0.]if i==0 else[10.25,3.,0.])]
            actions+=['Trim=Yes','Join='+('Yes'if join else'No'),'Enter']
            result['empty_'+name+('_join'if join else'_trim')]=case(actions,measure_jets=True)
    for first in [False,True]:
        for join in [False,True]:
            i=0 if first else 1;p=[-.25,0.,0.]if first else[10.25,3.,0.]
            sources=[pick(i,p),'Point',[10.,3.,2.]]if first else['Point',[0.,0.,2.],pick(i,p)]
            result['empty_point_'+('curve_first'if first else'point_first')+('_join'if join else'_trim')]=case(['BlendStart=CurveEnd']+sources+['Flip',pick(i,p),'Trim=Yes','Join='+('Yes'if join else'No'),'Enter'],measure_jets=True)
    for first in [False,True]:
        portion=['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],[-1.,0.,0.]]
        sources=portion+[pick(1,[10.25,3.,0.])]if first else[pick(1,[10.25,3.,0.])]+portion
        result['empty_portion_'+('first'if first else'second')+'_join']=case(['BlendStart=PickPoint']+sources+['Trim=Yes','Join=Yes','Enter'],measure_jets=True)
    return result

def holdouts(case,pick,a,b,arc,cubic):
    import copy
    result={}
    def run(name,actions,curves=None):
        result[name]=case(['BlendStart=CurveEnd']+actions+['Enter'],curves=curves,measure_jets=True)
    for name,source in [('arc',arc),('cubic',cubic),('reverse',dict(type='line',start=a['end'],end=a['start']))]:
        for order in [2,4]:
            run('empty_'+name+'_g'+str(order),['Continuity_1=G'+str(order),pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Flip',pick(0,[-.25,0.,0.]),'Trim=Yes','Join=Yes'],[source,b])
    for name,flips in [('first',[0]),('second',[1]),('both',[0,1])]:
        actions=[pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.])]
        for i in flips:actions+=['Flip',pick(i,[-.25,0.,0.]if i==0 else[10.25,3.,0.])]
        run('empty_no_trim_'+name,actions+['Trim=No','Join=Yes'])
    for i in [0,1]:
        endpoint=[0.,0.,0.]if i==0 else[10.,3.,0.]
        moved=[-.01,0.,0.]if i==0 else[10.01,3.,0.]
        run('empty_near_endpoint_'+str(i),[pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Flip',pick(i,[-.25,0.,0.]if i==0 else[10.25,3.,0.]),'Trim=Yes','Join=Yes',dict(click=endpoint),moved])
    for order in [0,4]:
        run('empty_point_g'+str(order),['Continuity_1=G'+str(order),pick(0,[-.25,0.,0.]),'Point',[10.,3.,2.],'Flip',pick(0,[-.25,0.,0.]),'Trim=Yes','Join=Yes'])
    portion=['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],[-1.,0.,0.]]
    run('empty_portion_point', ['BlendStart=PickPoint']+portion+['Point',[10.,3.,2.],'Trim=Yes','Join=Yes'])
    recipe=copy.deepcopy(result['empty_no_trim_both']);recipe['actions'][-1]='Cancel';recipe['pending_redo']=True;result['empty_cancel']=recipe
    return result
