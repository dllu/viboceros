"""Selection overlap, coplanar mode sharing, and construction-plane cutters."""
from copy import deepcopy


def cases(box,xy,wall,overlap):
    result={}
    def add(name,sources,args='',**extra):
        case=dict(operation='split',preselect=False,sources=deepcopy(sources),first=[0],
            second=list(range(1,len(sources))),capture_split_topology=True)
        if args:case['arguments']=args
        case.update(extra);result['mesh_split_selection_'+name]=case
    add('intersect_mesh',[xy,overlap],'_CoplanarFaces=_Mesh',operation='intersect',first=[0,1],command_name='MeshIntersect')
    add('after_mesh_mode',[xy,overlap])
    add('keep_mode',[xy,overlap],'_CoplanarFaces=_Keep')
    add('intersect_after_keep',[xy,overlap],operation='intersect',first=[0,1],command_name='MeshIntersect')
    for name,second in [('both',[0,1]),('first',[0]),('second',[1])]:
        add('cross_'+name,[box,wall],first=[0,1],second=second,
            second_arguments='_PermitSplittingInputsWithEachOther',cross_inputs=True)
    add('curve_xz',[box,dict(kind='polyline',points=[[2.,0.,-1.],[2.,0.,3.]])],
        cplane=[[0.,0.,0.],[1.,0.,0.],[0.,0.,1.]])
    add('curve_polyline',[box,dict(kind='polyline',points=[[2.,-1.,0.],[2.,1.5,0.],[5.,1.5,0.]])])
    return result
