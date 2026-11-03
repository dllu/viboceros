"""Independent G1x valence, normal-gap, scale and affine controls."""
import math
if __package__:
    from .subd_continuity_probe import source as known_source,run_recipe
else:
    from subd_continuity_probe import source as known_source,run_recipe

SCALES=('050','075','0832','0834','101','102','125','150','200')
SCENES=('fan6','fan8','fan9','eight_asymmetric','seven_tilt','seven_small','seven_large')+tuple('seven_z'+s for s in SCALES)
SECTORS=('sector_tri7','sector_tri9','sector_mixed7','sector_mixed9','sector_mixed7_tilt')
CHARTS=('chart_corner1','chart_corner2','chart_corner3','chart_varied','chart_reversed','chart_mixed1','chart_mixed_varied')

def cases(sectors=False,charts=False):
    scenes=CHARTS if charts else SECTORS if sectors else SCENES
    return [dict(id=scene+'_'+str(mode),op='subd_g1x',scene=scene,mode=mode) for scene in scenes for mode in (0,1,3)]

def validate(op):
    if not isinstance(op,dict) or op not in cases()+cases(True)+cases(charts=True) or type(op.get('mode')) is not int:
        raise ValueError('fixed G1x recipe required')

def source(scene):
    if scene in CHARTS:
        recipe=source('sector_mixed7') if scene.startswith('chart_mixed') else known_source('seven_fan')
        faces=[]
        for i,face in enumerate(recipe['faces']):
            rotation=int(scene[-1]) if scene[-1] in '123' else i%len(face)
            face=face[-rotation:]+face[:-rotation] if rotation else face[:]
            if scene=='chart_reversed':face=face[:1]+list(reversed(face[1:]))
            faces.append(face)
        recipe['faces']=faces;return recipe
    if scene in SECTORS:
        n=9 if scene.endswith('9') else 7;points=[[0.,0.,0.4]]
        for i in range(n):
            a=2*math.pi*i/n;points.append([1.5*math.cos(a),1.5*math.sin(a),0.1*math.sin(2*a)])
        faces=[[0,i+1,(i+1)%n+1] for i in range(n)]
        if 'mixed' in scene:
            a=math.pi/n;points.append([3*math.cos(a),3*math.sin(a),0.3*math.cos(3*a)])
            faces[0]=[0,1,n+1,2]
        if scene.endswith('tilt'):points=[[1.5*x+0.2*z+1,0.8*y-3,2*z+0.1*x] for x,y,z in points]
        return dict(points=points,faces=faces)
    if scene.startswith('fan') or scene=='eight_asymmetric':
        n=8 if scene=='eight_asymmetric' else int(scene[3:]);points=[[0.,0.,0.4]]
        for i in range(n):
            a=2*math.pi*i/n;points.append([1.5*math.cos(a),1.5*math.sin(a),0.1*math.sin(2*a)])
        for i in range(n):
            a=2*math.pi*(i+0.5)/n;points.append([3*math.cos(a),3*math.sin(a),0.3*math.cos(3*a)])
        if scene=='eight_asymmetric':points[1]=[1.2,0.2,0.7];points[n+2][2]+=0.35
        return dict(points=points,faces=[[0,1+i,1+n+i,1+(i+1)%n] for i in range(n)],corners=list(range(n+1,2*n+1)))
    recipe=known_source('seven_fan')
    if scene=='seven_tilt':recipe['points']=[[1.5*x+0.2*z+1,0.8*y-3,2*z+0.1*x] for x,y,z in recipe['points']]
    elif scene in ('seven_small','seven_large'):
        scale=0.125 if scene=='seven_small' else 8.;recipe['points']=[[scale*c for c in p] for p in recipe['points']]
    else:
        scales=dict(zip(SCALES,[0.5,0.75,0.832,0.834,1.01,1.02,1.25,1.5,2.]));scale=scales[scene[7:]]
        recipe['points']=[[x,y,scale*z] for x,y,z in recipe['points']]
    return recipe

def run(op,host,iterations,tolerance):
    validate(op)
    return run_recipe(source(op['scene']),op['mode'],host,iterations,tolerance)
