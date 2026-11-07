"""Whole closed sources: prescribed picks, never measured output controls."""
CIRCLE=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
def pilot(case,pick,line):
    result={}
    for start in ['CurveEnd','PickPoint']:
        for name,point in [('right',[5.,0.,0.]),('top',[0.,5.,0.]),('left',[-5.,0.,0.])]:
            result['closed_'+start.lower()+'_'+name]=case(['BlendStart='+start,pick(0,point),pick(1,[10.25,3.,0.]),'Enter'],curves=[CIRCLE,line],measure_jets=True)
    result['closed_second']=case(['BlendStart=PickPoint',pick(1,[10.25,3.,0.]),pick(0,[0.,5.,0.]),'Enter'],curves=[CIRCLE,line],measure_jets=True)
    result['closed_point']=case(['BlendStart=PickPoint',pick(0,[0.,5.,0.]),'Point',[10.,3.,2.],'Enter'],curves=[CIRCLE,line],measure_jets=True)
    return result

def discovery(case,pick,line):
    result={}
    for start in ['CurveEnd','PickPoint']:
        for name,p in [('upper',[3.,4.,0.]),('lower',[3.,-4.,0.]),('far',[-4.,3.,0.])]:
            result['closed_discover_'+start.lower()+'_'+name]=case(['BlendStart='+start,pick(0,p),pick(1,[10.25,3.,0.]),'Enter'],curves=[CIRCLE,line],measure_jets=True)
    for name,extra in [('trim',['Trim=Yes']),('join',['Join=Yes']),('trim_join',['Trim=Yes','Join=Yes']),('flip',['Flip',pick(0,[0.,5.,0.])])]:
        result['closed_discover_'+name]=case(['BlendStart=PickPoint',pick(0,[0.,5.,0.]),pick(1,[10.25,3.,0.])]+extra+['Enter'],curves=[CIRCLE,line],measure_jets=True)
    return result

def verification(case,pick,line):
    import copy
    return {'verified_'+name:copy.deepcopy(recipe)for name,recipe in pilot(case,pick,line).items()}

def holdouts(case,pick,line):
    import math
    def nurbs(points,weights=None,degree=3,knots=None):
        return dict(type='nurbs',degree=degree,control_points=[dict(point=p,weight=w)for p,w in zip(points,weights or[1.]*len(points))],knots=knots or[0.]*(degree+1)+[1.]*(degree+1))
    w=math.sqrt(.5)
    ellipse=nurbs([[6.,0.,0.],[6.,3.,0.],[0.,3.,0.],[-6.,3.,0.],[-6.,0.,0.],[-6.,-3.,0.],[0.,-3.,0.],[6.,-3.,0.],[6.,0.,0.]],[1.,w,1.,w,1.,w,1.,w,1.],2,[0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.])
    periodic=nurbs([[-3.,0.,0.],[0.,3.,1.],[3.,0.,0.],[0.,-3.,-1.],[-3.,0.,0.],[0.,3.,1.],[3.,0.,0.]],[1.,2.,.7,1.5,1.,2.,.7],knots=list(map(float,range(11))))
    corner=nurbs([[0.,0.,0.],[-5.,5.,2.],[5.,5.,-2.],[0.,0.,0.]],[1.,2.,.5,1.])
    square=dict(type='polyline',vertices=[[0.,0.,0.],[0.,4.,0.],[4.,4.,0.],[4.,0.,0.],[0.,0.,0.]])
    def point(source,t):
        # Independent rational de Boor evaluation chooses prescribed input aims.
        p=source['degree'];u=source['knots'];c=source['control_points'];k=next(i for i in range(p,len(c))if u[i]<=t<u[i+1]);d=[[x*v['weight']for x in v['point']]+[v['weight']]for v in c[k-p:k+1]]
        for r in range(1,p+1):
            for j in range(p,r-1,-1):
                a=(t-u[k-p+j])/(u[k+1+j-r]-u[k-p+j]);d[j]=[(1-a)*x+a*y for x,y in zip(d[j-1],d[j])]
        return [x/d[p][3]for x in d[p][:3]]
    def make(source,aim,order=2,extra=None,reverse=False,view='Top'):
        sources=[pick(0,aim),pick(1,[10.25,3.,0.])]
        if reverse:sources.reverse()
        recipe=case(['BlendStart=CurveEnd','Continuity_1=G'+str(order),'Continuity_2=G'+str(order)]+sources+['Trim=No','Join=No']+(extra or[])+['Enter'],curves=[source,line],measure_jets=True)
        recipe['view']=view
        return recipe
    result={}
    for order in [0,1,3,4]:result['closed_order_g'+str(order)]=make(CIRCLE,[0.,5.,0.],order)
    for name,source,aim in [('ellipse',ellipse,[0.,3.,0.]),('periodic',periodic,point(periodic,4.3)),('corner',corner,point(corner,.25))]:
        for order in [3,4]:result['closed_'+name+'_g'+str(order)]=make(source,aim,order,view='Perspective'if name!='ellipse'else'Top')
    result['closed_polyline']=make(square,[0.,2.,0.])
    result['closed_join_without_trim']=make(CIRCLE,[0.,5.,0.],extra=['Join=Yes']);result['closed_join_without_trim']['actions'][0]='BlendStart=PickPoint'
    result['closed_reverse_join']=make(CIRCLE,[0.,5.,0.],extra=['Trim=Yes','Join=Yes'],reverse=True)
    result['closed_cancel']=make(CIRCLE,[0.,5.,0.],extra=['Trim=Yes','Join=Yes']);result['closed_cancel']['actions'][-1]='Cancel';result['closed_cancel']['pending_redo']=True
    result['closed_corner_contact']=make(corner,point(corner,.25),4,[dict(click=point(corner,.25)),[0.,0.,0.]],view='Perspective')
    result['closed_corner_flip_contact']=make(corner,point(corner,.25),4,[dict(click=point(corner,.25)),[0.,0.,0.],'Flip',pick(0,point(corner,.25))],view='Perspective')
    return result

def ownership(case,pick,line,cubic):
    import copy
    other=copy.deepcopy(CIRCLE);other['center']=[14.,0.,0.];other['radius']=3.
    def pair(actions,curves=None,order=2):
        return case(['BlendStart=CurveEnd','Continuity_1=G'+str(order),'Continuity_2=G'+str(order)]+actions+['Trim=No','Join=No'],curves=curves or[CIRCLE,line],measure_jets=True)
    result={}
    for name,sources,curves in [
      ('point',[pick(0,[0.,5.,0.]),'Point',[10.,3.,2.]],[CIRCLE,line]),
      ('two',[pick(0,[0.,5.,0.]),pick(1,[14.,3.,0.])],[CIRCLE,other]),
      ('self',[pick(0,[0.,5.,0.]),pick(0,[-5.,0.,0.])],[CIRCLE,line]),
    ]:
        for join in [False,True]:
            recipe=pair(sources,curves);recipe['actions']+=['Trim=Yes','Join=Yes','Enter']if join else['Enter'];result['closed_ownership_'+name+('_join'if join else'')]=recipe
    recipe=pair([pick(0,[0.,5.,0.]),pick(1,[10.25,3.,0.])],[CIRCLE,cubic],3);recipe['actions']+=['Trim=Yes','Join=Yes','Enter'];result['closed_ownership_curved_join']=recipe
    recipe=pair([pick(0,[0.,5.,0.]),pick(1,[10.25,3.,0.])],order=3);recipe['actions']+=[dict(click=[2.5,4.6,0.]),[3.5,6.,2.],dict(click=[0.,5.,0.]),[-5.,0.,0.],'Enter'];result['closed_shape_then_contact']=recipe
    return result

def self_flips(case,pick,line):
    result={}
    for name,point in [('first',[0.,5.,0.]),('second',[-5.,0.,0.])]:
        result['closed_self_flip_'+name]=case(['BlendStart=CurveEnd',pick(0,[0.,5.,0.]),pick(0,[-5.,0.,0.]),'Trim=No','Join=No','Flip',pick(0,point),'Enter'],curves=[CIRCLE,line],measure_jets=True)
    return result
