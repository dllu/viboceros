"""Command-only selection, remembered-option, and infinite-plane controls."""
from copy import deepcopy


def cases(box,xy,wall,overlap,combine,moved):
    result={}
    def add(name,sources,args='',**extra):
        value=dict(operation='split',preselect=False,sources=deepcopy(sources),
            first=[0],second=list(range(1,len(sources))),capture_split_topology=True)
        if args:value['arguments']=args
        value.update(extra)
        result['mesh_split_policy_'+name]=value
    add('set_zero',[combine(xy,wall)],'_Tolerance=_Zero',operation='intersect',command_name='MeshSelfIntersect')
    add('after_self_zero',[box,wall])
    add('set_document',[box,wall],'_Tolerance=_Document')
    add('intersect_after_document',[xy,wall],operation='intersect',first=[0,1],command_name='MeshIntersect')
    add('drop_cancel',[xy,overlap],'_CoplanarFaces=_Drop',cancel=True)
    add('intersect_after_drop',[xy,overlap],operation='intersect',first=[0,1],command_name='MeshIntersect')
    add('after_drop_cancel',[xy,overlap])
    add('ngons_no',[box,wall],'_CreateNgons=_No')
    add('after_ngons_no',[box,wall])
    add('discard_no_cancel',[box,wall],'_DiscardUnsplitMeshes=_No',cancel=True)
    add('after_discard_no',[box,moved(wall,[8.,0.,0.])])
    add('restore',[box,wall],'_Tolerance=_Auto _CoplanarFaces=_Keep _CreateNgons=_Yes _DiscardUnsplitMeshes=_Yes')
    for option in ['Tolerance','CoplanarFaces']:
        add(option.lower()+'_menu',[box,wall],'_'+option+' _Cancel',menu=option)
    for pre in [False,True]:
        add('brep_target_'+('pre'if pre else'post'),[dict(box,kind='brep_box'),wall],preselect=pre)
    add('cross_inputs',[box,wall],first=[0,1],second=[],second_arguments='_PermitSplittingInputsWithEachOther',cross_inputs=True)
    for name,sources,macro,plane in [
        ('plane_three_point',[box],'_IP _3Point 2,0,0 2,1,0 2,0,1','2,0,0,1,0,0'),
        ('plane_world_yz',[moved(box,[-2.,0.,0.])],'_IP _WorldYZ','WorldYZ'),
        ('plane_through_point',[box],'_IP _ThroughPoint 0,0,1','0,0,1,0,0,1'),
    ]:
        add(name,sources,second=[],second_arguments=macro,infinite_plane=plane)
    return result
