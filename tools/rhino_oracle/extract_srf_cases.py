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


def curved_picking_request():
    """Curved inputs, parameter fractions and views fixed before native capture."""
    parents=geometry_request()['operations']
    def part(name):return copy.deepcopy(next(op for op in parents if op['id']=='standard-'+name+'-pre-1')['sources'][0]['brep'])
    disjoint=next(op for op in request()['operations'] if op['id']=='disjoint-first-copy-0-current-0')
    sheet=copy.deepcopy(disjoint['sources'][0]['brep']['source']['parts'][1])
    def compound(source):return dict(brep=dict(source=dict(type='compound',parts=[source,copy.deepcopy(sheet)])))
    cylinder,partial=part('cylinder-band'),part('cylinder-partial')
    signed=part('signed-cubic')
    sphere=dict(source=dict(type='sphere',radius=3))
    torus=dict(source=dict(type='torus',radii=[5,1.5]))
    surface=dict(degree_u=2,degree_v=2,control_point_count_u=3,control_point_count_v=3,
        knots_u=[0,0,0,10,10,10],knots_v=[0,0,0,10,10,10],control_points=[
            dict(point=[i*5,j*5,.1*(i*5-5)**2+.15*(j*5-5)**2],weight=1+.2*i+.1*j)
            for j in range(3) for i in range(3)])
    warped=dict(source=dict(type='surface_face',surface=surface))
    cases=[]
    for view,fraction in [('Front',[.75,.5]),('Back',[.25,.5]),('Left',[.5,.5]),('Right',[.05,.5])]:
        for display in ('Shaded','Ghosted','Wireframe'):
            cases.append(('cylinder-'+view+'-'+display,cylinder,view,display,fraction))
    for view in ('Top','Bottom','Front'):
        for display in ('Shaded','Ghosted'):
            # In Front, U=.5 lies on the projected silhouette: rounding to a
            # native pixel can miss the exact face. Use a visible interior ray.
            cases.append(('torus-'+view+'-'+display,torus,view,display,[.3 if view=='Front' else .5,.125]))
        for display in ('Shaded','Wireframe'):
            cases.append(('sphere-'+view+'-'+display,sphere,view,display,[.25,.75]))
    for display in ('Shaded','Ghosted'):cases.append(('signed-'+display,signed,'Top',display,[.45,.4]))
    for view in ('Top','Front'):
        for display in ('Shaded','Ghosted'):cases.append(('warped-'+view+'-'+display,warped,view,display,[.3,.3]))
    cases.extend([('partial-Back',partial,'Back','Shaded',[.3,.5]),('partial-Right',partial,'Right','Ghosted',[.3,.5])])
    operations=[]
    def operation(name,source,view,display,steps,copying=False):
        return dict(op='extract_srf_command',id='curved-'+name,sources=[compound(copy.deepcopy(source))],
            components=[],copy=copying,output_current=copying,source_layer=True,undo_redo=True,
            pick='sequence',steps=steps,finish='Enter',view=view,display=display)
    def click(face,fraction,modifier='plain'):
        return dict(kind='click',component=[0,face],modifiers=modifier,fraction=fraction)
    for name,source,view,display,fraction in cases:
        operations.append(operation(name,source,view,display,[click(0,fraction)],display=='Ghosted'))
    for name,source,fraction in [('torus',torus,[.5,.125]),('sphere',sphere,[.25,.75])]:
        operations.append(operation(name+'-both',source,'Top','Shaded',[click(0,fraction),click(1,[.3,.3])]))
        operations.append(operation(name+'-remove',source,'Top','Shaded',[click(0,fraction),click(1,[.3,.3]),click(0,fraction,'ctrl')]))
    for name,source,view,fraction in [('cylinder',cylinder,'Front',[.75,.5]),('sphere',sphere,'Top',[.25,.75])]:
        reversed_source=copy.deepcopy(source);reversed_source['reversed']=True
        operations.append(operation(name+'-reversed',reversed_source,view,'Shaded',[click(0,fraction)]))
    return dict(protocol_version=1,iterations=1,operations=operations)


if __name__=='__main__':print(json.dumps(request(),indent=2))
