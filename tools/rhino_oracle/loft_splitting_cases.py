"""Independent loft profiles, style choices, and both topology policies."""
import copy


def request(smoke=False):
    from .surface_apex_tangent_cases import sources,curve
    profiles=[(name,s['boundaries'][0]['curve'])for name,s,_ in sources() if name not in ['hole','tilted']]
    profiles += [('open_line',curve([[0.,0.,0.],[2.,0.,0.]],degree=1)),
                 ('open_kink',curve([[0.,0.,0.],[2.,0.,0.],[2.,1.,0.]],degree=1))]
    operations=[]
    for index,(name,profile) in enumerate(profiles):
        curves=[]
        for scale,z,dx,dy in [(1.,0.,0.,0.),(1.25,1.,.25,.5),(.75,3.,-.5,.25),(1.5,6.,.5,-.25)]:
            section=copy.deepcopy(profile)
            for cp in section['control_points']:
                x,y,_=cp['point'];cp['point']=[scale*x+dx,scale*y+dy,z]
            curves.append(section)
        for style in ['normal','loose','tight','straight','uniform']:
            for closed in ([False,True] if name in ['circle','corners','open_kink'] else [False]):
                for crease in [False,True]:
                    for split in [False,True]:
                        operations.append(dict(op='loft_splitting',id='loft-splitting-{}-{}-{}{}{}'.format(name,style,int(closed),int(crease),int(split)),
                            curves=copy.deepcopy(curves),style=style,closed=closed,crease_splitting=crease,
                            split_at_tangents=split,preselect=bool((index+int(crease)+int(split)+int(closed))%2)))
    if smoke:
        ids=['loft-splitting-open_kink-normal-000','loft-splitting-open_kink-straight-011',
             'loft-splitting-capsule-normal-001','loft-splitting-circle-normal-111']
        operations=[next(o for o in operations if o['id']==key)for key in ids]
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-8),operations=operations)


def diagnostics():
    query=request();by_id={o['id']:o for o in query['operations']};operations=[]
    ids=['loft-splitting-circle-normal-000','loft-splitting-circle_speed-normal-000',
         'loft-splitting-capsule_seam-normal-011','loft-splitting-corners-normal-110',
         'loft-splitting-open_kink-straight-111']
    for key in ids:
        for natural in [False,True]:
            op=copy.deepcopy(by_id[key]);op.update(id=key+('-natural'if natural else'-default'),natural=natural,inspection=True)
            operations.append(op)
        op=by_id[key]
        operations.append(dict(op='loft',id=key+'-sdk',curves=copy.deepcopy(op['curves']),style=op['style'],
            closed=op['closed'],command=False,sample_geometry=True))
    query['operations']=operations
    return query


def reorder_request():
    query=request();by_id={o['id']:o for o in query['operations']};operations=[]
    for name in ['circle','capsule_seam']:
        original=by_id['loft-splitting-'+name+'-normal-000']
        for order in [[2,0,3,1],[3,2,1,0]]:
            for flip in [None,0,1]:
                for closed in [False,True]:
                    op=copy.deepcopy(original)
                    op['curves']=[copy.deepcopy(original['curves'][i])for i in order]
                    if flip is not None:
                        c=op['curves'][flip];c['control_points'].reverse();c['knots']=[-x for x in reversed(c['knots'])]
                    op.update(id='loft-reorder-{}-{}-{}-{}'.format(name,''.join(map(str,order)),flip,int(closed)),closed=closed,inspection=False)
                    operations.append(op)
    query['operations']=operations
    return query


def angle_request():
    from .surface_apex_tangent_cases import threshold_request
    query=request();templates={o['id']:o for o in query['operations']};operations=[]
    profiles=[o['source']['boundaries'][0]['curve']for o in threshold_request()['operations']if not o['solid']and not o['split_at_tangents']]
    for index,profile in enumerate(profiles):
        curves=[]
        for scale,z,dx,dy in [(1.,0.,0.,0.),(1.25,1.,.25,.5),(.75,3.,-.5,.25),(1.5,6.,.5,-.25)]:
            c=copy.deepcopy(profile)
            for cp in c['control_points']:
                x,y,_=cp['point'];cp['point']=[scale*x+dx,scale*y+dy,z]
            curves.append(c)
        for style in ['normal','straight']:
            for crease in [False,True]:
                for split in [False,True]:
                    op=copy.deepcopy(templates['loft-splitting-capsule-'+style+'-0'+str(int(crease))+str(int(split))])
                    op.update(id='loft-angle-{}-{}-{}{}'.format(index,style,int(crease),int(split)),curves=copy.deepcopy(curves))
                    operations.append(op)
    query['operations']=operations
    return query


def manual_diagnostics():
    query=request();by_id={o['id']:o for o in query['operations']};operations=[]
    for name,seam in [('circle',1),('capsule_seam',4)]:
        original=copy.deepcopy(by_id['loft-splitting-'+name+'-normal-000'])
        original.update(id=original['id']+'-inspection',inspection=True)
        operations.append(original)
        curves=copy.deepcopy(original['curves']);last=curves[-1];n=2*seam
        last['control_points']=last['control_points'][n:-1]+last['control_points'][:n+1]
        last['knots']=[k+seam for k in last['knots']]
        operations.append(dict(op='loft',id=original['id']+'-manual-sdk',curves=curves,
                               style='normal',closed=False,command=False,sample_geometry=True))
    query['operations']=operations
    return query


def seam_diagnostics():
    query=request();by_id={o['id']:o for o in query['operations']};operations=[]
    original=by_id['loft-splitting-capsule_seam-normal-000']
    for reset in [False,True]:
        curves=copy.deepcopy(original['curves']);last=curves[-1];n=10
        last['control_points']=last['control_points'][n:-1]+last['control_points'][:n+1]
        if not reset:last['knots']=[k+5. for k in last['knots']]
        operations.append(dict(op='loft',id='loft-manual-left-reset-'+str(int(reset)),curves=curves,
                               style='normal',closed=False,command=False,sample_geometry=True))
    for count in [3,4]:
        for transformed in [False,True]:
            op=copy.deepcopy(by_id['loft-splitting-corners-straight-000'])
            op.update(id='loft-polyline-{}-{}'.format(count,int(transformed)),inspection=True)
            op['curves']=op['curves'][:count]
            if not transformed:
                for i,c in enumerate(op['curves']):
                    for p,xy in zip(c['control_points'],[[0.,0.],[1.,0.],[1.,1.],[0.,1.],[0.,0.]]):p['point']=[xy[0]*(1.+i),xy[1]*(1.+i),float(i*2)]
            operations.append(op)
    query['operations']=operations
    return query


def basis_diagnostics():
    query=request();by_id={o['id']:o for o in query['operations']};operations=[]
    original=by_id['loft-splitting-capsule_seam-normal-000']
    for shifted in [[],[0],[3],[0,1,2,3]]:
        curves=copy.deepcopy(original['curves'])
        for i in shifted:curves[i]['knots']=[k+5. for k in curves[i]['knots']]
        operations.append(dict(op='loft',id='loft-basis-shift-'+(''.join(map(str,shifted))or'none'),curves=curves,
                               style='normal',closed=False,command=False,sample_geometry=True,inspect_inputs=True))
    for count in [3,4]:
        for explicit in [False,True]:
            op=copy.deepcopy(by_id['loft-splitting-corners-straight-000'])
            op.update(id='loft-polyline-options-{}-{}'.format(count,int(explicit)),inspection=True,explicit_options=explicit,preselect=True)
            op['curves']=op['curves'][:count];operations.append(op)
        operations.append(dict(op='loft',id='loft-polyline-legacy-'+str(count),curves=copy.deepcopy(op['curves']),
                               style='straight',closed=False,command=True,sample_geometry=True))
    query['operations']=operations
    return query


def simplified_diagnostics():
    query=basis_diagnostics();operations=[]
    for op in query['operations']:
        if op['id']in ['loft-basis-shift-none','loft-basis-shift-3']:
            op=copy.deepcopy(op);op.update(id=op['id']+'-simplified',inspect_simplified=True);operations.append(op)
    for op in seam_diagnostics()['operations'][:2]:
        op=copy.deepcopy(op);op.update(id=op['id']+'-simplified',inspect_inputs=True,inspect_simplified=True);operations.append(op)
    from pathlib import Path
    import json
    old=json.loads((Path(__file__).resolve().parent/'fixtures/loft_command.json').read_text())
    original=copy.deepcopy(next(op for op in old['operations']if op['id']=='command-polyline-straight'))
    original['id']='loft-old-polyline-legacy';operations.append(original)
    operations.append(dict(op='loft_splitting',id='loft-old-polyline-explicit',curves=copy.deepcopy(original['curves']),
                           style='straight',closed=False,crease_splitting=True,split_at_tangents=False,preselect=True,inspection=True))
    query['operations']=operations
    return query


def reorder_inspection():
    query=reorder_request()
    for op in query['operations']:op.update(id=op['id']+'-inspection',inspection=True)
    return query


def tangent_jets():
    query=request();source=next(op for op in query['operations']if op['id']=='loft-splitting-circle-normal-000')
    operations=[]
    for index,curve in enumerate(source['curves']):
        for reverse in [False,True]:
            for seam in range(4):
                parameter=float(seam-4 if reverse else seam)
                operations.append(dict(op='curve_native',id='loft-tangent-{}-{}-{}'.format(index,int(reverse),seam),
                    curve=dict(type='nurbs',**copy.deepcopy(curve)),reversed=reverse,
                    edit=dict(kind='seam',parameter=parameter),differential_only=True,sided_parameters=[parameter]))
    query['operations']=operations
    return query


def chart_diagnostics():
    """Change one property of the original straight-polyline command fixture."""
    from pathlib import Path
    import json,math
    old=json.loads((Path(__file__).resolve().parent/'fixtures/loft_command.json').read_text())
    original=next(o for o in old['operations']if o['id']=='command-polyline-straight')['curves']
    variants=[]
    for name in ['original','domain','translate','aspect','monotone','height','offsets','four','rotate','reverse']:
        curves=copy.deepcopy(original)
        if name=='four':
            curves.append(copy.deepcopy(curves[0]))
            for p in curves[-1]['control_points']:p['point']=[1.5*p['point'][0],1.5*p['point'][1],6.]
        for i,c in enumerate(curves):
            if name=='domain':c['knots']=[4.*k for k in c['knots']]
            if name=='reverse':
                c['control_points'].reverse();c['knots']=[-k for k in reversed(c['knots'])]
            for p in c['control_points']:
                x,y,z=p['point']
                if name=='translate':x-=2.;y-=1.
                if name=='aspect':x*=2.
                if name=='monotone'and i==2:x*=3.;y*=3.
                if name=='height':z=[0.,1.,3.][i]
                if name=='offsets':x+=[0.,.25,-.5][i];y+=[0.,.5,.25][i]
                if name=='rotate':x,y=math.cos(.37)*x-math.sin(.37)*y,math.sin(.37)*x+math.cos(.37)*y
                p['point']=[x,y,z]
        variants.append((name,curves))
    query=request();query['operations']=[]
    for name,curves in variants:
        for crease in [False,True]:
            query['operations'].append(dict(op='loft_splitting',id='loft-chart-{}-{}'.format(name,int(crease)),
                curves=curves,style='straight',closed=False,crease_splitting=crease,
                split_at_tangents=False,preselect=True,inspection=True))
    return query


def domain_holdouts():
    """Ruled SDK surfaces expose each prepared section without interpolation."""
    query=request()
    original=next(o for o in query['operations']if o['id']=='loft-splitting-capsule_seam-straight-000')
    operations=[]
    for name in ['scaled','seam1','seam5','elliptic']:
        for shifted in [False,True]:
            curves=[copy.deepcopy(original['curves'][i])for i in [0,3]]
            last=curves[-1]
            seam={'seam1':1,'seam5':5}.get(name,0)
            if seam:
                n=2*seam
                last['control_points']=last['control_points'][n:-1]+last['control_points'][:n+1]
            if name=='elliptic':
                for p in last['control_points']:p['point'][0]*=2.
            if shifted:last['knots']=[k+(float(seam)if seam else 5.)for k in last['knots']]
            operations.append(dict(op='loft',id='loft-domain-{}-{}'.format(name,int(shifted)),
                curves=curves,style='straight',closed=False,command=False,
                sample_geometry=True,inspect_inputs=True))
    query['operations']=operations
    return query


def domain_compatibility():
    query=domain_holdouts()
    for op in query['operations']:
        op['id']+='-compatible'
        op['inspect_compatible']=True
    return query


def domain_preparation():
    query=domain_compatibility()
    for op in query['operations']:
        op['id']+='-preparation'
        op['inspect_preparation']=True
    return query


def domain_matching():
    query=domain_preparation()
    for op in query['operations']:
        op['id']+='-matching'
        op['inspect_matching']=True
    return query


def domain_aspect_holdouts():
    query=domain_matching();operations=[]
    for op in query['operations']:
        if not any(op['id'].startswith('loft-domain-'+name+'-1-')for name in ['seam1','seam5']):continue
        for index,scale in enumerate([.25,.5,2.,4.]):
            case=copy.deepcopy(op);case['id']+='-aspect-'+str(index)
            for p in case['curves'][-1]['control_points']:p['point'][0]*=scale
            operations.append(case)
    query['operations']=operations
    return query


def domain_geometric_matching():
    # Keep the earlier entry-wise scaling captures, including their shared seam
    # endpoint transformation. New affine variants copy each control separately.
    query=domain_aspect_holdouts()
    for op in query['operations']:
        op['id']+='-g2'
        op['inspect_geometric_matching']=True
    for op in domain_matching()['operations']:
        if not any(op['id'].startswith('loft-domain-'+name+'-1-')for name in ['seam1','seam5']):continue
        for index,scale in enumerate([.25,.5,2.,4.]):
            case=copy.deepcopy(op);case['id']+='-affine-'+str(index)+'-g2'
            last=case['curves'][-1]
            last['control_points']=[copy.deepcopy(p)for p in last['control_points']]
            for p in last['control_points']:p['point'][0]*=scale
            case['inspect_geometric_matching']=True
            query['operations'].append(case)
    return query


def domain_fastpath_holdouts():
    query=request();templates={o['id']:o for o in query['operations']};operations=[]
    for name,values in [('capsule_seam',[0.,.25,1.,2.5,3.5,5.75,6.]),
                        ('circle',[0.,.25,1.75,3.,4.])]:
        source=templates['loft-splitting-'+name+'-straight-000']
        for shifted in [False,True]:
            curves=[copy.deepcopy(source['curves'][i])for i in [0,3]]
            old=sorted(set(curves[-1]['knots']));mapping=dict(zip(old,values))
            assert len(old)==len(values)
            curves[-1]['knots']=[mapping[k]+(5. if shifted else 0.)for k in curves[-1]['knots']]
            operations.append(dict(op='loft',id='loft-fastpath-{}-{}'.format(name,int(shifted)),
                curves=curves,style='straight',closed=False,command=False,sample_geometry=True,
                inspect_inputs=True,inspect_compatible=True,inspect_preparation=True,
                inspect_matching=True,inspect_geometric_matching=True))
    query['operations']=operations
    return query


def domain_speed_samples():
    query=domain_geometric_matching()
    query['operations']=[query['operations'][i]for i in [0,3,8,11]]
    for op in query['operations']:
        op['id']+='-speeds'
        op['inspect_curve_speeds']=True
    return query


def domain_speed_jets():
    query=domain_speed_samples()
    for op in query['operations']:
        op['id']+='-jets'
        op['inspect_curve_speed_jets']=True
    return query


def domain_quadrature():
    query=domain_speed_jets()
    for op in query['operations']:
        op['id']+='-quadrature'
        op['inspect_quadrature']=True
    return query


def open_basis_holdouts():
    """Separate open spline curvature breaks from the closed-section policy."""
    query=request();operations=[]
    points=[[[0.,0.,0.],[.3,.5,0.],[.7,-.5,0.],[1.,0.,0.]],
            [[0.,0.,1.],[.3,-.5,1.],[.7,1.,1.],[1.,0.,1.]]]
    for label in ['weighted','unit','translated-domain','cubic']:
        curves=[]
        for i in range(2):
            degree=3 if label=='cubic'else 2
            weights=([2.,.7,1.,1.]if i==0 else[1.,1.,1.,3.])if label=='weighted'else[1.]*4
            controls=[dict(point=p,weight=w)for p,w in zip(points[i],weights)]
            interior=.3 if i==0 else .7
            if label=='cubic':controls.insert(2,dict(point=[.5,.25,float(i)],weight=1.))
            knots=[0.]*(degree+1)+[interior]+[1.]*(degree+1)
            if label=='translated-domain'and i==1:knots=[5.+2.*k for k in knots]
            curves.append(dict(degree=degree,control_points=controls,knots=knots))
        operations.append(dict(op='loft',id='loft-open-basis-'+label,curves=curves,style='straight',closed=False,
            command=False,sample_geometry=True,inspect_inputs=True,inspect_compatible=True,
            inspect_matching=True,inspect_geometric_matching=True))
    query['operations']=operations
    return query


def orientation_holdouts():
    """Symmetric/asymmetric rings separate chart sense from degree and count."""
    import math
    query=request();square=chart_diagnostics()['operations'][0]['curves'][0]
    circle=next(o for o in query['operations']if o['id']=='loft-splitting-circle-straight-000')['curves'][0]
    peak=[(1.,0.),(2.,2.),(1.,4.)]
    variants=[
        ('square-peak',square,peak,False,False,'straight'),
        ('square-flat',square,[(1.,0.),(1.,2.),(1.,4.)],False,False,'straight'),
        ('square-valley',square,[(2.,0.),(1.,2.),(2.,4.)],False,False,'straight'),
        ('square-unequal-ends',square,[(1.,0.),(2.,2.),(.5,4.)],False,False,'straight'),
        ('square-off-midplane',square,[(1.,0.),(2.,1.),(1.,4.)],False,False,'straight'),
        ('square-four-midplane',square,[(1.,0.),(2.,2.),(1.5,3.),(1.,4.)],False,False,'straight'),
        ('square-five-midplane',square,[(1.,0.),(1.5,1.),(2.,2.),(1.5,3.),(1.,4.)],False,False,'straight'),
        ('square-four-off-midplane',square,[(1.,0.),(2.,1.),(1.5,3.),(1.,4.)],False,False,'straight'),
        ('square-centered',square,peak,True,False,'straight'),
        ('square-tilted',square,peak,False,True,'straight'),
        ('circle-peak',circle,peak,False,False,'straight'),
        ('circle-valley',circle,[(2.,0.),(1.,2.),(2.,4.)],False,False,'straight'),
        ('circle-flat',circle,[(1.,0.),(1.,2.),(1.,4.)],False,False,'straight'),
        ('circle-off-midplane',circle,[(1.,0.),(2.,1.),(1.,4.)],False,False,'straight'),
        ('circle-peak-normal',circle,peak,False,False,'normal'),
        ('square-peak-normal',square,peak,False,False,'normal'),
    ]
    operations=[]
    for name,base,rings,centered,tilted,style in variants:
        curves=[]
        for radius,height in rings:
            controls=[]
            for cp in base['control_points']:
                x,y,_=cp['point'];x=radius*(x-(.5 if centered else 0.));y=radius*(y-(.5 if centered else 0.));z=height
                if tilted:x,z=math.cos(.37)*x+math.sin(.37)*z,-math.sin(.37)*x+math.cos(.37)*z
                controls.append(dict(point=[x,y,z],weight=cp['weight']))
            curves.append(dict(degree=base['degree'],knots=list(base['knots']),control_points=controls))
        operations.append(dict(op='loft_splitting',id='loft-orientation-'+name,curves=curves,style=style,closed=False,
            crease_splitting=False,split_at_tangents=False,preselect=True,inspection=True,orientation_inspection=True))
    for name in ['circle','corners']:
        for style in ['normal','straight']:
            op=copy.deepcopy(next(o for o in query['operations']if o['id']=='loft-splitting-{}-{}-100'.format(name,style)))
            op.update(id='loft-orientation-closed-{}-{}'.format(name,style),inspection=True,orientation_inspection=True)
            operations.append(op)
    query['operations']=operations
    return query


def orientation_witnesses():
    query=orientation_holdouts()
    query['operations']=[query['operations'][i]for i in [0,2,3,4,5,9,10,14]]
    offsets=copy.deepcopy(next(o for o in chart_diagnostics()['operations']if o['id']=='loft-chart-offsets-0'))
    offsets['orientation_inspection']=True;query['operations'].append(offsets)
    for op in query['operations']:
        op['id']+='-witness';op['orientation_witness']=True
    return query


def closed_orientation_regular():
    query=orientation_holdouts();base={o['id']:o for o in query['operations']};operations=[]
    for name in ['circle-peak','square-centered']:
        for style in ['normal','straight']:
            for reverse in [False,True]:
                op=copy.deepcopy(base['loft-orientation-'+name]);op.update(id='loft-closed-regular-{}-{}-{}'.format(name,style,int(reverse)),style=style,closed=True)
                if reverse:
                    for c in op['curves']:
                        c['control_points'].reverse();c['knots']=[-k for k in reversed(c['knots'])]
                operations.append(op)
    query['operations']=operations
    return query
