"""Interleaved whole objects and temporary portions in one native getter."""
ORDER={}
sequences={
    'portion_first':([['range',0],['whole',1]],[[0,.2,.7]],[1]),
    'sandwich':([['whole',1],['range',0],['whole',2],['range',1]],[[0,.1,.3],[0,.6,.9]],[1,2]),
    'portions_first':([['range',0],['range',1],['whole',1]],[[0,.1,.3],[0,.6,.9]],[1]),
    'whole_owner_last':([['range',0],['whole',0]],[[0,.2,.7]],[0]),
    'reverse_portions':([['whole',2],['range',1],['whole',1],['range',0]],[[0,.1,.3],[0,.6,.9]],[2,1]),
}
for name,(inputs,ranges,whole) in sequences.items():
    for output in ['Points','PointCloud']:
        ORDER['range_order_'+name+'_'+output.lower()]=dict(
            sources=[dict(kind=kind,mode=None,grips=[]) for kind in ['rational','line','circle']],
            whole=whole,order=[0,1,2],output=output,layer='Input',ranges=ranges,input_order=inputs)

GRIPS={}
for output in ['Points','PointCloud']:
    GRIPS['range_order_grips_'+output.lower()]=dict(
        sources=[dict(kind=kind,mode='control' if i==2 else None,grips=[])
            for i,kind in enumerate(['rational','line','circle'])],
        whole=[1],order=[0,1,2],output=output,layer='Input',ranges=[[0,.2,.7]],
        input_order=[['range',0],['whole',1]],range_grips=True)

EDGES={}
for kind in ['brep_box','brep_surface']:
    for output in ['Points','PointCloud']:
        EDGES['range_order_'+kind+'_'+output.lower()]=dict(
            sources=[dict(kind=k,mode=None,grips=[]) for k in [kind,'line']],
            whole=[1],order=[0,1],output=output,layer='Input',edge_ranges=[[0,0,.2,.7]],
            range_whole_first=False,input_order=[['range',0],['whole',1]])

DISPLAYS={}
for output in ['Points','PointCloud']:
    DISPLAYS['range_order_unpicked_display_'+output.lower()]=dict(
        sources=[dict(kind=kind,mode='control' if i==2 else None,grips=[])
            for i,kind in enumerate(['rational','line','circle'])],
        whole=[1],order=[0,1,2],output=output,layer='Input',ranges=[[0,.2,.7]],
        input_order=[['range',0],['whole',1]])
    for mode in ['control','edit']:
        DISPLAYS['range_order_hidden_'+mode+'_'+output.lower()]=dict(
            sources=[dict(kind='rational',mode=None,grips=[]),dict(kind='line',mode=None,grips=[]),
                dict(kind='circle',mode='control',grips=[0,2]),dict(kind='circle',mode=mode,grips=[])],
            whole=[1],order=[0,1,2,3],output=output,layer='Input',ranges=[[0,.2,.7]],
            input_order=[['range',0],['whole',1]],range_grips=True,getter=True,clear_initial_grips=True)
