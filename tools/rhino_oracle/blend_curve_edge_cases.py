"""Prescribed naked surface-edge contacts and default cross-edge directions."""
def surface(points):
    return dict(type='surface',degree_u=1,degree_v=1,control_point_count_u=2,control_point_count_v=2,control_points=[dict(point=p,weight=1.)for p in points],knots_u=[0.,0.,1.,1.],knots_v=[0.,0.,1.,1.])
PLANE=surface([[-5.,-5.,0.],[-5.,0.,0.],[0.,-5.,0.],[0.,0.,0.]])
OTHER=surface([[10.,3.,0.],[10.,8.,0.],[15.,3.,0.],[15.,8.,0.]])
def edge(i,p,index=1):return dict(curve=i,edge=index,pick=p)
def pilot(case,pick,line):
    first=edge(0,[0.,-2.5,0.]);second=pick(1,[10.25,3.,0.])
    def make(actions,curves=None):return case(actions+['Trim=No','Join=No','Enter'],curves=curves or[PLANE,line],measure_jets=True)
    result={
      'edge_plane_end':make(['BlendStart=CurveEnd',first,second]),
      'edge_plane_pick':make(['BlendStart=PickPoint',first,second]),
      'edge_second':make(['BlendStart=CurveEnd',second,first]),
      'edge_point':make(['BlendStart=CurveEnd',first,'Point',[10.,3.,0.]]),
      'edge_two_planes':make(['BlendStart=CurveEnd',first,edge(1,[10.,5.5,0.],3)],[PLANE,OTHER]),
    }
    for order in [0,1,3,4]:result['edge_plane_g'+str(order)]=make(['BlendStart=CurveEnd','Continuity_1=G'+str(order),'Continuity_2=G'+str(order),first,second])
    return result

def selection(case,pick,line):
    a=edge(0,[0.,-2.5,0.],2);b=edge(1,[10.,5.5,0.],0)
    def make(actions,curves):return case(actions+['Trim=No','Join=No','Enter'],curves=curves,measure_jets=True)
    return {
      'edge_restricted_first':make(['BlendStart=CurveEnd','Edges',a,pick(1,[10.25,3.,0.])],[PLANE,line]),
      'edge_restricted_two':make(['BlendStart=CurveEnd','Edges',a,'Edges',b],[PLANE,OTHER]),
      'edge_restricted_pick':make(['BlendStart=PickPoint','Edges',a,'Edges',b],[PLANE,OTHER]),
      'edge_restricted_point':make(['BlendStart=PickPoint','Edges',a,'Point',[10.,3.,0.]],[PLANE,line]),
    }

def tensor(xs,ys,z):
    controls=[[x,y,z(i,j)]for j,x in enumerate(xs)for i,y in enumerate(ys)]
    return dict(type='surface',degree_u=len(ys)-1,degree_v=len(xs)-1,control_point_count_u=len(ys),control_point_count_v=len(xs),control_points=[dict(point=p,weight=1.)for p in controls],knots_u=[0.]*len(ys)+[1.]*len(ys),knots_v=[0.]*len(xs)+[1.]*len(xs))
CURVED=tensor([-5.,-4.,-2.,0.],[-5.,0.],lambda i,j:[0.,1.,0.,0.][j])
SKEW=tensor([-5.,-4.,-2.,0.],[-5.,-3.,-1.,0.],lambda i,j:[[0.,1.,0.,0.],[1.,2.,1.,1.],[0.,1.,2.,0.],[0.,0.,0.,0.]][j][i])
def discovery(case,pick,line):
    a=edge(0,[0.,-2.5,0.],2);b=edge(1,[10.,5.5,0.],0)
    def make(actions,curves=None,**kw):return case(actions+['Enter'],curves=curves or[PLANE,OTHER],measure_jets=True,**kw)
    def pair(order=2,extra=None,curves=None):return make(['BlendStart=CurveEnd','Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',a,b]+(extra or[]),curves)
    result={'surface_plane_g'+str(g):pair(g)for g in range(5)}
    result.update({
      'surface_plane_angles':pair(extra=['AtAngle_1=30','AtAngle_2=-45']),
      'surface_plane_point':make(['BlendStart=CurveEnd','Edges',a,'Point',[10.,3.,2.]],curves=[PLANE,line]),
      'surface_mixed':make(['BlendStart=CurveEnd','Edges',a,'Curves',pick(1,[10.25,3.,0.])],curves=[PLANE,line]),
      'surface_mixed_second':make(['BlendStart=CurveEnd',pick(1,[10.25,3.,0.]),'Edges',a],curves=[PLANE,line]),
      'surface_plane_slide':pair(extra=[dict(click=[0.,-2.5,0.]),[0.,-1.,0.]]),
      'surface_plane_shape':pair(extra=[dict(click=[3.16,-2.46,0.]),[4.,-2.46,0.]]),
      'surface_plane_symmetry':pair(extra=[dict(click=[3.16,-2.46,0.],shift=True),[4.,-2.46,0.]]),
      'surface_cancel':make(['Edges',a,b,'Cancel'],pending_redo=True),
    })
    for order in [1,2,3,4]:result['surface_curved_g'+str(order)]=pair(order,curves=[CURVED,OTHER])
    result['surface_skew_g2']=pair(curves=[SKEW,OTHER])
    return result
