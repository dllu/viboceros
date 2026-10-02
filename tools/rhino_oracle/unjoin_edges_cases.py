"""Independent source recipes for public B-rep edge separation."""
import copy
import json


def request():
    operations=[]
    box=dict(source=dict(type='box',min=[0.,0.,0.],max=[2.,3.,5.]))
    tube=dict(source=dict(type='solid_tube',radii=[2.,5.],height=8.))
    def add(name,source,edges):
        operations.append(dict(op='brep_unjoin_edges',id=name,source=copy.deepcopy(source),edges=edges))
    for name,edges in [('empty',[]),('one',[0]),('corner-two',[0,3]),('corner-three',[0,3,8]),('cap',[0,1,2,3]),('cap-reverse',[3,2,1,0]),('duplicate',[0,0,3,0]),('all',list(range(12)))]:
        add('box-'+name,box,edges)
    reversed_box=dict(box,reversed=True)
    add('box-reversed',reversed_box,[0,3])
    split_box=dict(box,splits=[[0,[.5,1.5]]])
    add('box-split-one',split_box,[0]); add('box-split-chain',split_box,[0,12,13])
    pair=copy.deepcopy(box); pair['source']['keep_faces']=[0,2]
    add('pair-joined',pair,[0]); add('pair-naked',pair,[1]); add('pair-mixed',pair,[1,0,2])
    for name,edges in [('outer-seam',[1]),('inner-seam',[4]),('outer-circle',[0]),('inner-circle',[3]),('bottom',[0,3]),('all',list(range(6))),('mixed-reverse',[5,4,3,2,1,0])]:
        add('tube-'+name,tube,edges)
    add('sphere-seam',dict(source=dict(type='sphere',radius=5.)),[0])
    triangles=dict(source=dict(type='mesh_brep',vertices=[[0.,0.,0.],[2.,0.,0.],[0.,2.,0.],[0.,-2.,0.],[0.,0.,2.]],faces=[[0,1,2],[1,0,3],[0,1,4]]))
    add('nonmanifold-shared',triangles,[0]); add('nonmanifold-all',triangles,list(range(7)))
    other=copy.deepcopy(box); other['source']['min'][0]+=10.; other['source']['max'][0]+=10.
    compound=dict(source=dict(type='compound',parts=[box,other]))
    add('compound-one',compound,[0]); add('compound-caps',compound,[0,1,2,3,12,13,14,15])
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__=='__main__': print(json.dumps(request(),indent=2))
