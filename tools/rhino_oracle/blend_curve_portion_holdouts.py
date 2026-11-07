"""Independent bounded-source jets, edits and ownership holdouts."""
def at(source,t):
    net=[[p['point'][i]*p['weight'] for i in range(3)]+[p['weight']] for p in source['control_points']]
    while len(net)>1:net=[[(1.-t)*a+t*b for a,b in zip(p,q)]for p,q in zip(net,net[1:])]
    return [x/net[0][3]for x in net[0][:3]]
def holdouts(case,pick,arc,line,rational,quintic):
    first=['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],[-1.,0.,0.]]
    second=['SubCrv',pick(1,[10.25,3.,0.]),[11.,3.,0.],[14.,3.,0.]]
    def make(actions,curves=None):return case(['BlendStart=CurveEnd']+actions+['Enter'],curves=curves,measure_jets=True)
    result={
      'portion_slide_join':make(first+[pick(1,[10.25,3.,0.]),'Trim=Yes','Join=Yes',dict(click=[-1.,0.,0.]),[-2.,0.,0.]]),
      'portion_clamp':make(first+[pick(1,[10.25,3.,0.]),'Trim=No','Join=No',dict(click=[-1.,0.,0.]),[0.,0.,0.]]),
      'portion_shape':make(first+[pick(1,[10.25,3.,0.]),'Trim=No','Join=No',dict(click=[1.,0.,0.]),[3.,0.,0.]]),
      'portion_flip':make(first+[pick(1,[10.25,3.,0.]),'Trim=No','Join=No','Flip','Enter']),
      'portion_both_no_trim_join':make(first+second+['Trim=No','Join=Yes']),
      'portion_whole_first_join':make([pick(0,[-.25,0.,0.])]+second+['Trim=Yes','Join=Yes']),
      'portion_arc_g2':make(['SubCrv',pick(0,[-5.,-5.,0.]),[-3.,-1.,0.],[0.,0.,0.],pick(1,[10.25,3.,0.]),'Trim=No','Join=No'],[arc,line]),
    }
    for name,source,start,end,order in [('rational_g3',rational,.2,.8,3),('reverse_quintic_g4',quintic,.8,.2,4)]:
        result['portion_'+name]=make(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'SubCrv',pick(0,at(source,.02)),at(source,start),at(source,end),pick(1,[10.25,3.,0.]),'Trim=No','Join=No'],[source,line])
    return result
