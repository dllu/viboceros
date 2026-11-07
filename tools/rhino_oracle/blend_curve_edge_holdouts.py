"""Prescribed surface metric, angle, spatial and ownership holdouts."""
import copy
if __package__:from .blend_curve_edge_cases import PLANE,OTHER,CURVED,SKEW,edge
else:from blend_curve_edge_cases import PLANE,OTHER,CURVED,SKEW,edge

def holdouts(case,pick,line):
    a=edge(0,[0.,-2.5,0.],2);b=edge(1,[10.,5.5,0.],0)
    def pair(surface,extra=None,order=2):
        return case(['Continuity_1=G'+str(order),'Continuity_2=G'+str(order),'Edges',a,b]+(extra or[])+['Enter'],curves=[surface,OTHER],measure_jets=True)
    rational=copy.deepcopy(CURVED)
    for j,w in enumerate([1.,.4,2.,1.]):
        for i in range(2):rational['control_points'][2*j+i]['weight']=w
    skew=copy.deepcopy(SKEW)
    for j,offset in enumerate([0.,2.,-1.,0.]):
        for i in range(4):skew['control_points'][4*j+i]['point'][1]+=offset
    result={
      'surface_rational_g2':pair(rational),
      'surface_nonorthogonal_g2':pair(skew),
      'surface_curved_angle':pair(CURVED,['AtAngle_1=37','AtAngle_2=-23']),
      'surface_nonorthogonal_angle':pair(skew,['AtAngle_1=-41','AtAngle_2=12']),
      'surface_angle_g3':pair(PLANE,['AtAngle_1=-32','AtAngle_2=48'],3),
      'surface_angle_g4':pair(PLANE,['AtAngle_1=61','AtAngle_2=-17'],4),
      'surface_cancel_complete':case(['Edges',a,b,'Cancel'],curves=[PLANE,OTHER],measure_jets=True,pending_redo=True),
    }
    def rotate(p):return [.8*p[0]-.6*p[2]+2.,p[1]+1.,.6*p[0]+.8*p[2]-3.]
    for name,surface in [('surface_spatial_plane',PLANE),('surface_spatial_curved',CURVED)]:
        recipe=copy.deepcopy(pair(surface));recipe['view']='Perspective'
        for source in recipe['curves']:
            for cp in source['control_points']:cp['point']=rotate(cp['point'])
        for action in recipe['actions']:
            if isinstance(action,dict)and 'pick'in action:action['pick']=rotate(action['pick'])
        result[name]=recipe
    for second in [False,True]:
        for name,options in [('trim',['Trim=Yes','Join=No']),('join',['Trim=Yes','Join=Yes']),('join_whole',['Trim=No','Join=Yes']),('flip',['Flip','Enter'])]:
            actions=[pick(1,[12.,3.,0.]),'Edges',a]if second else['Edges',a,'Curves',pick(1,[12.,3.,0.])]
            result['surface_mixed_'+name+('_second'if second else'')]=case(['BlendStart=PickPoint']+actions+options+['Enter'],curves=[PLANE,line],measure_jets=True)
    return result
