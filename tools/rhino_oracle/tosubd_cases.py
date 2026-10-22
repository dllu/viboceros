"""Fixed ToSubD conversion inputs, independent of command implementation."""
from copy import deepcopy
import math

QUAD = [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
STRIP = [[0., 0., 0.], [2., 0., 0.], [4., 0., 1.], [0., 3., 0.], [2., 3., 0.], [4., 3., 1.]]
SEAM = [STRIP[i] for i in [0, 1, 4, 3, 1, 2, 5, 4]]


def mesh(vertices=None, faces=None, **options):
    return dict(kind='mesh', vertices=vertices or deepcopy(QUAD),
                faces=faces or [[0, 1, 2, 3]], **options)


def surface(count_u=4, count_v=4, degree=3, rational=False):
    def knots(count):
        return [0.] * (degree + 1) + [float(i) / (count - degree) for i in range(1, count - degree)] + [1.] * (degree + 1)
    points = [dict(point=[float(u), float(v), float((u - 1) * (v - 1)) / 3.],
                   weight=1. + (0.3 if rational and (u + v) % 3 == 1 else 0.))
              for v in range(count_v) for u in range(count_u)]
    return dict(kind='surface', degree_u=degree, degree_v=degree,
                control_point_count_u=count_u, control_point_count_v=count_v,
                control_points=points, knots_u=knots(count_u), knots_v=knots(count_v))


DISCOVERY = {}
for name, source in [
    ('quad', mesh()), ('triangle', mesh(faces=[[0, 1, 2]])),
    ('strip', mesh(STRIP, [[0, 1, 4, 3], [1, 2, 5, 4]])),
    ('seam', mesh(SEAM, [[0, 1, 2, 3], [4, 5, 6, 7]])),
    ('ngon', mesh(STRIP, [[0, 1, 4, 3], [1, 2, 5, 4]], ngons=[dict(vertices=[0, 1, 2, 5, 4, 3], faces=[0, 1])])),
    ('bicubic', surface()), ('bilinear', surface(2, 2, 1)),
    ('rational', surface(rational=True)), ('multispan', surface(6, 5)),
    ('box', dict(kind='box')), ('box_face', dict(kind='box', selected_faces=[0])),
    ('extrusion', dict(kind='extrusion')), ('trimmed', dict(kind='trimmed')),
]:
    DISCOVERY['tosubd_' + name + '_default'] = dict(sources=[source])
for location in [False, True]:
    for corners in [False, True]:
        for creases in [False, True]:
            key = 'tosubd_mesh_' + '_'.join(str(int(v)) for v in [location, corners, creases])
            DISCOVERY[key] = dict(sources=[mesh(SEAM, [[0, 1, 2, 3], [4, 5, 6, 7]])],
                                  options=dict(UseMesh='Location' if location else 'ControlPoints',
                                               MeshCorners=corners, MeshCreases=creases, DeleteInput=True))
for name, source in [('bilinear', surface(2, 2, 1)), ('bicubic', surface()), ('rational', surface(rational=True)), ('multispan', surface(6, 5))]:
    for location in [False, True]:
        for corners in [False, True]:
            key = 'tosubd_' + name + '_' + str(int(location)) + str(int(corners))
            DISCOVERY[key] = dict(sources=[source], options=dict(UseSurface='Location' if location else 'ControlPoints', SurfaceCorners=corners, DeleteInput=True))
DISCOVERY.update(
    tosubd_mesh_keep=dict(sources=[mesh()], options=dict(DeleteInput=False)),
    tosubd_surface_keep=dict(sources=[surface()], options=dict(DeleteInput=False)),
    tosubd_mesh_preselected=dict(sources=[mesh()], preselected=True),
    tosubd_surface_preselected=dict(sources=[surface()], preselected=True),
    tosubd_mixed=dict(sources=[mesh(), surface()]),
    tosubd_cancel=dict(sources=[mesh()], cancel=True),
)


DEFAULT_OPTIONS = dict(UseMesh='ControlPoints', MeshCreases=False, MeshCorners=False,
                       UseSurface='Location', SurfaceCorners=False, DeleteInput=False)
SCENES = {}
for key, definition in DISCOVERY.items():
    recipe = deepcopy(definition)
    recipe['options'] = dict(DEFAULT_OPTIONS, **definition.get('options', {}))
    SCENES['tosubd_main_' + key[len('tosubd_'):]] = recipe
SCENES['tosubd_main_cancel_selection'] = dict(sources=[mesh()], cancel_selection=True)


SURFACE_DETAILS = {}
for name, definition in [('bilinear_warp', surface(2, 2, 1)), ('quadratic', surface(3, 3, 2)), ('cubic', surface()), ('quartic', surface(5, 5, 4)), ('multispan', surface(6, 5)), ('rational', surface(rational=True))]:
    definition = deepcopy(definition)
    for i, control in enumerate(definition['control_points']):
        u, v = control['point'][:2]
        control['point'][2] = (u*u + 2*v*v + u*v) / 7. + (0.4 if i%3 == 1 else 0.)
    for location in [False, True]:
        for corners in [False, True]:
            options = dict(DEFAULT_OPTIONS, UseSurface='Location' if location else 'ControlPoints', SurfaceCorners=corners)
            SURFACE_DETAILS['tosubd_surface_' + name + '_' + str(int(location)) + str(int(corners))] = dict(sources=[definition], options=options)
for kind in ['sphere_surface', 'cylinder_surface', 'torus_surface']:
    for location in [False, True]:
        for corners in [False, True]:
            SURFACE_DETAILS['tosubd_' + kind + '_' + str(int(location)) + str(int(corners))] = dict(sources=[dict(kind=kind)], options=dict(DEFAULT_OPTIONS, UseSurface='Location' if location else 'ControlPoints', SurfaceCorners=corners))
for faces in [[0], [1], [0, 1], [0, 1, 2, 3, 4, 5]]:
    for delete in [False, True]:
        SURFACE_DETAILS['tosubd_box_faces_' + ''.join(str(i) for i in faces) + '_' + str(int(delete))] = dict(sources=[dict(kind='box', selected_faces=faces)], options=dict(DEFAULT_OPTIONS, DeleteInput=delete))
SURFACE_DETAILS['tosubd_mesh_face'] = dict(sources=[mesh(STRIP, [[0, 1, 4, 3], [1, 2, 5, 4]], selected_faces=[0])], options=deepcopy(DEFAULT_OPTIONS))


SURFACE_EDGES = {}
periodic = surface(11, 4)
periodic['knots_u'] = [float(i) for i in range(-3, 12)]
periodic['control_points'] = [dict(point=[(2.+.3*v)*math.cos((u%8)*math.pi/4.),(2.+.3*v)*math.sin((u%8)*math.pi/4.),.2*v*v],weight=1.)for v in range(4)for u in range(11)]
unclamped = surface(6,5)
unclamped['knots_u'] = [float(i) for i in range(-3,7)]
nonuniform=surface(6,5)
nonuniform['knots_u']=[0.]*4+[.15,.8]+[1.]*4
for name, source in [('periodic',periodic),('unclamped',unclamped),('nonuniform',nonuniform),('cone',dict(kind='cone_surface'))]:
    for location in [False,True]:
        for corners in [False,True]:
            SURFACE_EDGES['tosubd_edge_'+name+'_'+str(int(location))+str(int(corners))]=dict(sources=[source],options=dict(DEFAULT_OPTIONS,UseSurface='Location'if location else'ControlPoints',SurfaceCorners=corners))
for delta in [0.,1e-10,1e-8,1e-6,1e-4]:
    source=surface();source['control_points'][5]['point'][2]+=delta
    for absolute in [1e-6,1e-3]:
        SURFACE_EDGES['tosubd_near_bilinear_'+str(delta)+'_'+str(absolute)]=dict(sources=[source],options=deepcopy(DEFAULT_OPTIONS),absolute_tolerance=absolute)


FEATURES = {}
curved_unclamped = deepcopy(unclamped)
for c in curved_unclamped['control_points']:
    x,y,_=c['point'];c['point'][2]=(x*x+y*y)/5.
friendly = surface(6,4)
for j in range(4):
    for i,x in enumerate([0.,1./3.,1.,2.,8./3.,3.]):
        friendly['control_points'][j*6+i]['point']=[x,float(j),x*j*j/7.]
kink=surface(3,2,1)
for c in kink['control_points']:
    x,y,_=c['point'];c['point'][2]=abs(x-1.)
for name,source in [('cone',dict(kind='cone_surface')),('sphere',dict(kind='sphere_surface')),('cylinder',dict(kind='cylinder_surface')),('torus',dict(kind='torus_surface')),('unclamped_curved',curved_unclamped),('friendly',friendly),('kink',kink)]:
    for location in [False,True]:
        FEATURES['tosubd_features_'+name+'_'+str(int(location))]=dict(sources=[source],record_features=True,options=dict(DEFAULT_OPTIONS,UseSurface='Location'if location else'ControlPoints'))


POLES = {}
profile=[(1.,0.),(1.,1.),(0.,1.),(-1.,1.),(-1.,0.),(-1.,-1.),(0.,-1.),(1.,-1.),(1.,0.)]
for name,source in [('elevated_cone2',dict(kind='cone_surface',degree_v=2)),('elevated_cone3',dict(kind='cone_surface',degree_v=3)),('hemisphere',dict(kind='hemisphere_surface'))]:
    for location in [False,True]:
        POLES['tosubd_poles_'+name+'_'+str(int(location))]=dict(sources=[source],record_features=True,sdk=True,options=dict(DEFAULT_OPTIONS,UseSurface='Location'if location else'ControlPoints'))
for name,radial,height in [('paraboloid',[0.,1.,2.],[0.,0.,-1.]),('cusp',[0.,0.,2.],[0.,1.,2.])]:
    source=dict(kind='surface',degree_u=2,degree_v=2,control_point_count_u=9,control_point_count_v=3,knots_u=[0.,0.,0.,1.,1.,2.,2.,3.,3.,4.,4.,4.],knots_v=[0.,0.,0.,1.,1.,1.],control_points=[dict(point=[radial[v]*x,radial[v]*y,height[v]],weight=1. if i%2==0 else math.sqrt(.5))for v in range(3)for i,(x,y)in enumerate(profile)])
    for location in [False,True]:
        POLES['tosubd_poles_'+name+'_'+str(int(location))]=dict(sources=[source],record_features=True,sdk=True,options=dict(DEFAULT_OPTIONS,UseSurface='Location'if location else'ControlPoints'))


try:
    from .tosubd_extrusion_cases import recipes as extrusion_recipes, pipe_recipes, conversion_edge_recipes
except (ImportError, ValueError):
    from tosubd_extrusion_cases import recipes as extrusion_recipes, pipe_recipes, conversion_edge_recipes
EXTRUSIONS = extrusion_recipes(DEFAULT_OPTIONS)
PIPES = pipe_recipes(DEFAULT_OPTIONS)
CONVERSION_EDGES = conversion_edge_recipes(DEFAULT_OPTIONS)


def all_cases():
    result = {}
    for cases in [DISCOVERY, SCENES, SURFACE_DETAILS, SURFACE_EDGES, FEATURES, POLES, EXTRUSIONS, PIPES, CONVERSION_EDGES]:
        result.update(cases)
    return result


def request(discovery=False, surface_details=False, surface_edges=False, features=False, poles=False, extrusions=False, pipes=False, conversion_edges=False):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='tosubd_command', id=k, case=k) for k in (DISCOVERY if discovery else SURFACE_DETAILS if surface_details else SURFACE_EDGES if surface_edges else FEATURES if features else POLES if poles else EXTRUSIONS if extrusions else PIPES if pipes else CONVERSION_EDGES if conversion_edges else SCENES)])


def validate(operation):
    try:
        valid = (isinstance(operation, dict) and set(operation) == {'op', 'id', 'case'}
                 and operation['op'] == 'tosubd_command' and operation['case'] in all_cases()
                 and operation['id'] == operation['case'])
    except TypeError:
        valid = False
    if not valid:
        raise ValueError('fixed ToSubD recipe required')
