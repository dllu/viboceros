"""Fixed public subdivision-ellipsoid recipes in an owned idle document."""

def scene(actions, around=8, vertical=4, cap='Tri', **kw):
    return dict(actions=['AroundFaces='+str(around), 'VerticalFaces='+str(vertical),
                         'CapFaceStyle='+cap]+actions, **kw)

DISCOVERY = {
    'ellipsoid_default': dict(actions=[[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.]),
    'ellipsoid_grid': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.]),
    'ellipsoid_numbers': scene([[0.,0.,0.],5.,3.,2.]),
    'ellipsoid_radius': scene(['Radius',[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.]),
    'ellipsoid_quad': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.], cap='Quad'),
    'ellipsoid_odd': scene([[1.,2.,3.],[6.,2.,3.],[1.,5.,3.],2.],around=7,vertical=5),
    'ellipsoid_small': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.],around=3,vertical=2),
    'ellipsoid_diameter': scene(['Diameter',[-5.,0.,0.],[5.,0.,0.],[0.,3.,0.],4.]),
    'ellipsoid_corner': scene(['Corner',[-5.,-3.,0.],[5.,3.,0.],2.]),
    'ellipsoid_centered_corner': scene([[0.,0.,0.],'Corner',[5.,3.,0.],2.]),
    'ellipsoid_foci': scene(['FromFoci',[-4.,0.,0.],[4.,0.,0.],'MarkFoci=Yes',[0.,3.,0.],2.]),
    'ellipsoid_vertical': scene(['Vertical',[0.,0.,0.],[5.,0.,0.],[0.,0.,3.],2.]),
    'ellipsoid_spatial': scene([[1.,2.,3.],[4.,6.,3.],[0.,5.,5.],[2.,4.,10.]]),
    'ellipsoid_negative': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],-2.]),
    'ellipsoid_third_point': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],[8.,9.,2.]]),
    'ellipsoid_cancel': dict(actions=[[0.,0.,0.],'Cancel'],cancel=True),
}
OPTIONS = {
    'ellipsoid_options_foci_no': scene(['FromFoci',[-4.,0.,0.],[4.,0.,0.],'MarkFoci=No',[0.,3.,0.]]),
    'ellipsoid_options_foci_yes': scene(['FromFoci',[-4.,0.,0.],[4.,0.,0.],'MarkFoci=Yes',[0.,3.,0.]]),
    'ellipsoid_options_foci_spatial': scene(['FromFoci',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]]),
    'ellipsoid_options_foci_collinear': scene(['FromFoci',[-4.,0.,0.],[4.,0.,0.],[6.,0.,0.]]),
    'ellipsoid_options_diameter_vertical': scene(['Diameter','Vertical',[-5.,0.,0.],[5.,0.,0.],[0.,0.,3.],4.]),
    'ellipsoid_options_zero': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],0.]),
    'ellipsoid_options_zero_offset': scene([[1.,2.,3.],[6.,2.,3.],[1.,5.,3.],0.]),
    'ellipsoid_options_enter': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],'Enter']),
    'ellipsoid_options_third_diameter': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],'Diameter',4.]),
    'ellipsoid_options_locked': scene([[0.,0.,0.],5.,[3.,4.,0.],3.,[0.,0.,5.],2.]),
    'ellipsoid_options_locked_diameter': scene(['Diameter',[-5.,0.,0.],10.,[5.,0.,0.],6.,[0.,4.,0.],4.]),
    'ellipsoid_options_four_tri': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.],around=4,vertical=2),
    'ellipsoid_options_four_quad': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.],around=4,vertical=2,cap='Quad'),
    'ellipsoid_options_six_quad': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.],around=6,vertical=3,cap='Quad'),
    'ellipsoid_options_around_z': scene(['AroundCurve',{'curve':True},[0.,0.,2.],[5.,0.,2.],[0.,3.,2.],2.],curve=[[0.,0.,0.],[0.,0.,10.]]),
    'ellipsoid_options_around_x': scene(['AroundCurve',{'curve':True},[2.,0.,0.],[2.,5.,0.],[2.,0.,3.],2.],curve=[[0.,0.,0.],[10.,0.,0.]]),
    'ellipsoid_options_late_counts': scene([[0.,0.,0.],'AroundFaces=6',[5.,0.,0.],[0.,3.,0.],2.]),
    'ellipsoid_options_odd': dict(actions=['CapFaceStyle=Tri','AroundFaces=7','VerticalFaces=5',[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.]),
}
SHAPES = {}
for key, around, vertical, cap, radii in [
    ('unit_default',10,10,'Quad',[1.,1.,1.]),
    ('unit_grid',8,4,'Tri',[1.,1.,1.]),
    ('unit_odd',7,5,'Tri',[1.,1.,1.]),
    ('unit_small',3,2,'Tri',[1.,1.,1.]),
    ('long_first',10,10,'Quad',[20.,3.,2.]),
    ('long_second',10,10,'Quad',[5.,20.,2.]),
    ('long_third',10,10,'Quad',[5.,3.,20.]),
    ('equal_default',10,10,'Quad',[5.,5.,5.]),
    ('odd_long_first',7,5,'Tri',[20.,3.,2.]),
    ('odd_long_second',7,5,'Tri',[5.,20.,2.]),
    ('odd_long_third',7,5,'Tri',[5.,3.,20.]),
    ('five_seven',5,7,'Tri',[5.,3.,2.]),
    ('six_three',6,3,'Quad',[5.,3.,2.]),
    ('twelve_nine',12,9,'Quad',[5.,3.,2.]),
]:
    actions=['AroundFaces='+str(around),'VerticalFaces='+str(vertical)]
    if around%2==0: actions.append('CapFaceStyle='+cap)
    SHAPES['ellipsoid_shapes_'+key]=dict(actions=actions+[[0.,0.,0.],[radii[0],0.,0.],[0.,radii[1],0.],radii[2]])
SCENES = {}
for key in ['ellipsoid_default','ellipsoid_grid','ellipsoid_quad','ellipsoid_diameter',
            'ellipsoid_corner','ellipsoid_centered_corner','ellipsoid_spatial',
            'ellipsoid_negative','ellipsoid_third_point']:
    SCENES[key.replace('ellipsoid_', 'subd_ellipsoid_')] = DISCOVERY[key]
for key in ['foci_no','foci_yes','foci_spatial','foci_collinear','diameter_vertical',
            'zero_offset','locked','locked_diameter','four_tri','four_quad',
            'six_quad','around_z','around_x','odd']:
    SCENES['subd_ellipsoid_'+key] = OPTIONS['ellipsoid_options_'+key]
for key, value in SHAPES.items():
    SCENES[key.replace('ellipsoid_shapes_', 'subd_ellipsoid_shapes_')] = value
SCENES.update({
    'subd_ellipsoid_tilted': scene([[10.,20.,30.],[13.,24.,30.],[10.,20.,33.],2.],construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
    'subd_ellipsoid_offset_odd': dict(actions=['AroundFaces=7','VerticalFaces=5',[10000.,20000.,30000.],[10005.,20000.,30000.],[10000.,20003.,30000.],2.]),
    'subd_ellipsoid_diameter_point': scene(['Diameter',[-5.,0.,0.],[5.,0.,0.],[0.,3.,0.],[8.,9.,2.]]),
    'subd_ellipsoid_third_negative_point': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],[8.,9.,-2.]]),
    'subd_ellipsoid_cancel_center': dict(actions=['Cancel'],cancel=True),
    'subd_ellipsoid_cancel_first_axis': scene([[0.,0.,0.],'Cancel'],cancel=True),
    'subd_ellipsoid_cancel_second_axis': scene([[0.,0.,0.],[5.,0.,0.],'Cancel'],cancel=True),
    'subd_ellipsoid_cancel_third_axis': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],'Cancel'],cancel=True),
    'subd_ellipsoid_enter_third_axis': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],'Enter'],cancel=True),
    'subd_ellipsoid_zero_third_axis': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],0.],no_output=True),
    'subd_ellipsoid_zero_third_point': scene([[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],[8.,9.,0.]],no_output=True),
    'subd_ellipsoid_cancel_foci': scene(['FromFoci',[-4.,0.,0.],[4.,0.,0.],'Cancel'],cancel=True),
    'subd_ellipsoid_cancel_around': scene(['AroundCurve','Cancel'],cancel=True),
})

DETAILS = {
    'subd_ellipsoid_focal_default': dict(actions=['FromFoci',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]]),
    'subd_ellipsoid_focal_quad': scene(['FromFoci',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],cap='Quad'),
    'subd_ellipsoid_focal_odd': dict(actions=['AroundFaces=7','VerticalFaces=5','FromFoci',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]]),
    'subd_ellipsoid_focal_small': dict(actions=['AroundFaces=3','VerticalFaces=2','FromFoci',[-4.,0.,0.],[4.,0.,0.],[0.,3.,0.]]),
    'subd_ellipsoid_focal_offset': dict(actions=['AroundFaces=7','VerticalFaces=5','FromFoci',[9996.,20000.,30000.],[10004.,20000.,30000.],[10000.,20003.,30002.]]),
    'subd_ellipsoid_focal_toggle': scene(['FromFoci',[-4.,0.,0.],'MarkFoci',[4.,0.,0.],[0.,3.,0.]]),
    'subd_ellipsoid_focal_tilted': scene(['FromFoci',[6.,2.,3.],[9.,6.,3.],[12.,10.,3.]],construction_plane=[[1.,2.,3.],[.6,.8,0.],[0.,0.,1.]]),
    'subd_ellipsoid_zero_center': scene([0.,[5.,0.,0.],[0.,3.,0.],2.]),
    'subd_ellipsoid_zero_first': scene([[1.,2.,3.],0.,[4.,0.,3.],2.]),
    'subd_ellipsoid_zero_second': scene([[1.,2.,3.],[6.,2.,3.],0.,2.]),
    'subd_ellipsoid_count_defaults': dict(actions=['AroundFaces','Enter','VerticalFaces','Enter',[0.,0.,0.],[5.,0.,0.],[0.,3.,0.],2.]),
    'subd_ellipsoid_around_spatial': scene(['AroundCurve',{'curve':True},[0.,0.,2.],[5.,0.,7.],[0.,3.,10.],[1.,2.,4.]],curve=[[0.,0.,0.],[0.,0.,10.]]),
    'subd_ellipsoid_cancel_picked_guide': scene(['AroundCurve',{'curve':True},'Cancel'],curve=[[0.,0.,0.],[0.,0.,10.]],cancel=True),
}

DETAILS['subd_ellipsoid_zero_second']['actions'].append('Cancel')
DETAILS['subd_ellipsoid_zero_second']['cancel']=True
DETAILS['subd_ellipsoid_retry_second']=scene([[1.,2.,3.],[6.,2.,3.],0.,3.,[1.,5.,3.],2.])

def request(discovery=False, options=False, shapes=False, details=False):
    cases = DETAILS if details else SHAPES if shapes else OPTIONS if options else DISCOVERY if discovery else SCENES
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='subd_ellipsoid_command',id=k,case=k) for k in cases])

def validate(op):
    cases = dict(list(DISCOVERY.items())+list(OPTIONS.items())+list(SHAPES.items())+list(SCENES.items())+list(DETAILS.items()))
    try:
        valid = (isinstance(op,dict) and set(op)=={'op','id','case'}
                 and op['op']=='subd_ellipsoid_command' and op['case'] in cases
                 and op['id']==op['case'])
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed SubDEllipsoid recipe required')

def run(op,host,iterations,tolerance):
    validate(op)
    from subd_box_probe import run_scene
    return run_scene(op,host,iterations,tolerance,'SubDEllipsoid',dict(list(DISCOVERY.items())+list(OPTIONS.items())+list(SHAPES.items())+list(SCENES.items())+list(DETAILS.items()))[op['case']])
