"""Independent face extraction inputs; native outputs are never source recipes."""
import copy
import json
from .shrink_trimmed_cases import face_request,geometry_request


def request():
    parents=face_request()['operations']
    def source(name):
        return copy.deepcopy(next(op for op in parents if op['id']=='standard-'+name+'-faces-0')['sources'][0])
    joined,disjoint=source('joined'),source('disjoint')
    cases=[('joined-first',[joined],[[0,0]]),('joined-second',[joined],[[0,1]]),
        ('joined-both',[joined],[[0,0],[0,1]]),('joined-reverse-order',[joined],[[0,1],[0,0]]),
        ('disjoint-first',[disjoint],[[0,0]]),('disjoint-both',[disjoint],[[0,0],[0,1]]),
        ('mixed',[joined,disjoint],[[1,1],[0,0]]),('duplicate',[joined],[[0,0],[0,0]]),
        ('mixed-forward',[joined,disjoint],[[0,0],[1,1]]),('mixed-repeat',[joined,disjoint],[[1,1],[0,0]])]
    geometry=geometry_request()['operations']
    def part(name):
        return copy.deepcopy(next(op for op in geometry if op['id']=='standard-'+name+'-pre-1')['sources'][0]['brep'])
    curved=dict(brep=dict(source=dict(type='compound',parts=[part('cylinder-band'),part('signed-cubic')])))
    reversed_curved=copy.deepcopy(curved);reversed_curved['brep']['reversed']=True
    cases.extend([('curved-seam',[curved],[[0,0]]),('signed-rational',[curved],[[0,1]]),
        ('curved-reversed',[reversed_curved],[[0,0],[0,1]])])
    box=dict(brep=dict(source=dict(type='box',min=[0.,0.,0.],max=[2.,3.,4.])))
    cases.extend([('box-all',[box],[[0,f] for f in range(6)]),('box-subset',[box],[[0,0],[0,5],[0,2]])])
    operations=[]
    for name,sources,faces in cases:
        for copying in (False,True):
            for current in (False,True):
                operations.append(dict(op='extract_srf_command',id='%s-copy-%d-current-%d'%(name,copying,current),
                    sources=copy.deepcopy(sources),components=copy.deepcopy(faces),copy=copying,
                    output_current=current,source_layer=True,undo_redo=True))
    return dict(protocol_version=1,iterations=1,operations=operations)


def picking_request():
    """Mouse/keyboard inputs derived solely from explicit independent sources."""
    disjoint=copy.deepcopy(next(op for op in request()['operations'] if op['id']=='disjoint-first-copy-0-current-0')['sources'][0])
    joined=copy.deepcopy(disjoint)
    part=joined['brep']['source']['parts'][1]
    for control in part['source']['surface']['control_points']:control['point'][0]-=12
    part['source']['trim_bounds']=[[0,6],[0,10]]
    joined['brep']['joins']=[[1,7,True]]
    translated=copy.deepcopy(disjoint)
    for part in translated['brep']['source']['parts']:
        for control in part['source']['surface']['control_points']:control['point'][0]+=40
    def click(face,modifier='plain',source=0):return dict(kind='click',component=[source,face],modifiers=modifier)
    def window(first=1,last=29,modifier='plain',top=11,bottom=-1):
        return dict(kind='window',corners=[[first,top,0],[last,bottom,0]],modifiers=modifier)
    def key(value):return dict(kind='key',value=value)
    cases=[
        ('single',[disjoint],[click(0)],'Enter'),
        ('both',[disjoint],[click(0),click(1)],'Enter'),
        ('reverse',[disjoint],[click(1),click(0)],'Enter'),
        ('duplicate',[disjoint],[click(0),click(0)],'Enter'),
        ('remove',[disjoint],[click(0),click(1),click(0,'ctrl')],'Enter'),
        ('ctrl-unselected',[disjoint],[click(0),click(1,'ctrl')],'Enter'),
        ('ctrlshift',[disjoint],[click(0),click(1),click(0,'sub')],'Enter'),
        ('ctrlshift-add',[disjoint],[click(0),click(1,'sub')],'Enter'),
        ('cancel',[disjoint],[click(0)],'Cancel'),
        ('none',[disjoint],[click(0),key('None')],'Cancel'),
        ('none-empty',[disjoint],[key('None')],'Cancel'),
        ('rectangle',[disjoint],[window()],'Enter'),
        ('rectangle-remove',[disjoint],[click(0),click(1),window(1,9,'ctrl')],'Enter'),
        ('rectangle-sub',[disjoint],[click(0),click(1),window(1,9,'sub')],'Enter'),
        ('rectangle-cross',[disjoint],[window(9,1,top=6,bottom=4)],'Enter'),
        ('rectangle-shift',[disjoint],[click(0),window(21,29,'shift')],'Enter'),
        ('options',[disjoint],[click(0),key('Copy=Yes'),key('OutputLayer=Current')],'Enter'),
        ('options-reverse',[disjoint],[click(0),key('Copy=No'),key('OutputLayer=Input')],'Enter'),
        ('joined',[joined],[click(0),click(1)],'Enter'),
        ('joined-remove',[joined],[click(0),click(1),click(0,'ctrl')],'Enter'),
        ('mixed',[disjoint,translated],[click(0),click(1,source=1),click(1)],'Enter'),
        ('mixed-reverse',[disjoint,translated],[click(1,source=1),click(1),click(0)],'Enter'),
        ('mixed-rectangle',[disjoint,translated],[window(1,69)],'Enter'),
        ('mixed-remove',[disjoint,translated],[click(0),click(1,source=1),click(0,'ctrl')],'Enter'),
    ]
    operations=[]
    variants={'single','both','remove','cancel','rectangle','joined','mixed','mixed-rectangle'}
    for name,sources,steps,finish in cases:
        for copying,current in ([(False,False),(True,True)] if name in variants else [(name=='options-reverse',name=='options-reverse')]):
            operations.append(dict(op='extract_srf_command',id='mouse-%s-copy-%d-current-%d'%(name,copying,current),
                sources=copy.deepcopy(sources),components=[],copy=copying,output_current=current,
                source_layer=True,undo_redo=True,pick='sequence',steps=copy.deepcopy(steps),finish=finish))
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__=='__main__':print(json.dumps(request(),indent=2))
