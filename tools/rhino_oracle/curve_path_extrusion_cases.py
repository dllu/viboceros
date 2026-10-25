"""Independent profile/path pairs for tangent and crease splitting."""
import copy
import math


def request():
    try:
        from .surface_apex_tangent_cases import sources, curve
    except (ImportError, ValueError, SystemError):
        from surface_apex_tangent_cases import sources, curve
    profiles = [(name, source['boundaries'][0]['curve']) for name,source,_ in sources() if name != 'hole']
    profiles += [('open_line',curve([[0.,0.,0.],[2.,0.,0.]],degree=1)),
                 ('open_kink',curve([[0.,0.,0.],[2.,0.,0.],[2.,2.,0.]],degree=1))]
    paths = [
        ('line',curve([[5.,7.,11.],[6.,9.,17.]],degree=1)),
        ('corner',curve([[5.,7.,11.],[5.,7.,14.],[6.,9.,17.]],degree=1)),
        ('rational',curve([[5.,7.,11.],[6.,9.,13.],[6.,9.,17.]],[1.,.6,1.])),
    ]
    circle = copy.deepcopy(next(p for name,p in profiles if name == 'circle'))
    for cp in circle['control_points']:
        x,y,z=cp['point'];cp['point']=[5.+2.*x,7.,11.+2.*y]
    paths.append(('closed',circle))
    scenes=[(name,profile,paths[0][0],paths[0][1])for name,profile in profiles]
    scenes += [(name,profile,pname,path)for name,profile in profiles if name in ['capsule','circle','open_line','open_kink'] for pname,path in paths[1:]]
    operations=[]
    for index,(name,profile,pname,path) in enumerate(scenes):
        for crease in [False,True]:
            for split in [False,True]:
                for solid in [False,True]:
                    operations.append(dict(op='curve_path_extrusion',id='curve-path-{}-{}-{}{}{}'.format(name,pname,int(crease),int(split),int(solid)),
                        profile=copy.deepcopy(profile),path=copy.deepcopy(path),crease_splitting=crease,
                        split_at_tangents=split,solid=solid,delete_input=bool((index+int(split)+int(solid))%2),
                        preselect=bool((index+int(crease)+int(solid))%2)))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-8),operations=operations)


def continuity_request():
    """Threshold angles in both directions and rational curvature joins on paths."""
    from .surface_apex_tangent_cases import curve, threshold_request
    query=request();query['operations']=[]
    line=curve([[0.,0.,0.],[2.,0.,0.]],degree=1)
    vertical=curve([[5.,7.,11.],[5.,7.,17.]],degree=1)
    scenes=[]
    for axis in ['profile','path']:
        for angle in [0.,.05,.099,.1,.101,.25,1.01]:
            bend=math.tan(math.radians(angle))
            if axis=='profile':
                profile=curve([[0.,0.,0.],[2.,0.,0.],[4.,2.*bend,0.]],degree=1);path=vertical
            else:
                profile=line;path=curve([[5.,7.,11.],[5.,7.,14.],[5.,7.+3.*bend,17.]],degree=1)
            scenes.append(('{}-{:g}'.format(axis,angle),profile,path,False))
    arc_line=curve([[5.,7.,11.],[5.,7.,12.],[5.,8.,12.],[5.,9.,12.],[5.,10.,12.]],
                   [1.,math.sqrt(.5),1.,1.,1.],degree=2)
    circle=next(o['profile'] for o in request()['operations'] if o['id']=='curve-path-circle-line-000')
    for name,profile in [('open',line),('circle',circle)]:
        scenes.append(('path-curvature-'+name,profile,arc_line,True))
    for op in threshold_request()['operations']:
        if not op['solid'] and not op['split_at_tangents']:
            scenes.append(('profile-curvature-'+op['id'].split('-')[-2],
                           op['source']['boundaries'][0]['curve'],vertical,True))
    for index,(name,profile,path,caps) in enumerate(scenes):
        for crease in [False,True]:
            for split in [False,True]:
                for solid in ([False,True] if caps else [False]):
                    query['operations'].append(dict(op='curve_path_extrusion',id='curve-path-continuity-{}-{}{}{}'.format(name,int(crease),int(split),int(solid)),
                        profile=copy.deepcopy(profile),path=copy.deepcopy(path),crease_splitting=crease,
                        split_at_tangents=split,solid=solid,delete_input=bool((index+int(split)+int(solid))%2),
                        preselect=bool((index+int(crease)+int(solid))%2)))
    return query
