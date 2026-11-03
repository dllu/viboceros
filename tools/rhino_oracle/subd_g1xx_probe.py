"""Fixed G1xx controls across valence, geometry and parameter-chart profiles."""
import math
if __package__:
    from .subd_g1x_probe import SCENES,SECTORS,CHARTS,source as known_source
    from .subd_continuity_probe import run_recipe
else:
    from subd_g1x_probe import SCENES,SECTORS,CHARTS,source as known_source
    from subd_continuity_probe import run_recipe

EVEN=('even_tri6','even_mixed6','even_mixed6_charts','trapezo6','trapezo6_tilt','trapezo8_asymmetric','trapezo6_charts')
EVEN_CHARTS=('even_corner1','even_corner2','even_corner3','even_varied')
CREASES=('trapezo6_first_even1','trapezo6_first_even2')
DEGREE=('trapezo6_first_crease','dart8')

GROUPS=dict(base=SCENES+SECTORS+CHARTS,even=EVEN,charts=EVEN_CHARTS,creases=CREASES,degree=DEGREE)

def cases(group='base'):
    return [dict(id=scene+'_4',op='subd_g1xx',scene=scene,mode=4) for scene in GROUPS[group]]

def validate(op):
    if not isinstance(op,dict) or op not in [candidate for group in GROUPS for candidate in cases(group)] or type(op.get('mode')) is not int:
        raise ValueError('fixed G1xx recipe required')

def source(scene):
    if scene=='trapezo6_first_crease':
        recipe=source('trapezo6_first_even1');recipe['creases']=[[3,4],[4,5]];return recipe
    if scene=='dart8':
        recipe=known_source('eight_asymmetric');recipe['creases']=[[0,1]];return recipe
    if scene in CREASES:
        recipe=source('trapezo6');rotation=3 if scene.endswith('1') else 2
        recipe['faces']=[face[rotation:]+face[:rotation] for face in recipe['faces']]
        recipe['creases']=[[4,5]] if scene.endswith('1') else [[3,4]]
        return recipe
    if scene in EVEN_CHARTS:
        recipe=known_source('eight_asymmetric');faces=[]
        for i,face in enumerate(recipe['faces']):
            rotation=int(scene[-1]) if scene[-1] in '123' else i%4
            faces.append(face[rotation:]+face[:rotation])
        recipe['faces']=faces;return recipe
    if scene not in EVEN:return known_source(scene)
    n=8 if scene.startswith('trapezo8') else 6
    if scene.startswith('even'):
        points=[[0.,0.,0.4]]
        for i in range(n):
            a=2*math.pi*i/n;points.append([1.5*math.cos(a),1.5*math.sin(a),0.1*math.sin(2*a)])
        faces=[[0,i+1,(i+1)%n+1] for i in range(n)]
        if 'mixed' in scene:
            a=math.pi/n;points.append([3*math.cos(a),3*math.sin(a),0.3*math.cos(3*a)])
            faces[0]=[0,1,n+1,2]
    else:
        points=[[0.,0.,2.],[0.,0.,-2.]]
        for i in range(2*n):
            a=math.pi*i/n;points.append([2*math.cos(a),2*math.sin(a),0.4 if i%2==0 else -0.4])
        r=lambda i:2+i%(2*n)
        faces=[[0,r(2*i),r(2*i+1),r(2*i+2)] for i in range(n)]
        faces += [[1,r(2*i+1),r(2*i),r(2*i-1)] for i in range(n)]
        if scene.endswith('tilt'):points=[[1.5*x+0.2*z+1,0.8*y-3,2*z+0.1*x] for x,y,z in points]
        if scene.endswith('asymmetric'):points[3]=[1.2,0.8,-0.1];points[0][0]+=0.3
    if scene.endswith('charts'):
        faces=[face[i%len(face):]+face[:i%len(face)] for i,face in enumerate(faces)]
    return dict(points=points,faces=faces)

def run(op,host,iterations,tolerance):
    validate(op)
    return run_recipe(source(op['scene']),4,host,iterations,tolerance)
