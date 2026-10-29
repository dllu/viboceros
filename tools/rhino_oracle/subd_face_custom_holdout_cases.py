"""Independent coherent UVWs, non-quads, winding and public-transform witnesses."""
from copy import deepcopy
try:
    from .subd_face_custom_cases import CASES as BASE
except (ImportError,ValueError):
    from subd_face_custom_cases import CASES as BASE

CASES={}
for name in ['strip','grid-L','neighbor','ring-partial','disjoint']:
    c=deepcopy(BASE['combined-'+name]);s=c['source']
    s['textures']=[[[.25*s['vertices'][v][0],-.5*s['vertices'][v][1],
                      .125*(s['vertices'][v][0]+2*s['vertices'][v][1])]for v in f]for f in s['faces']]
    CASES['coherent-'+name]=c
c=deepcopy(CASES['coherent-strip']);c['source']['textures']=[[[3.25,-7.5,.875]for v in f]for f in c['source']['faces']]
CASES['constant-uvw']=c
c=deepcopy(CASES['coherent-strip']);c['source']['textures'][1]=deepcopy(c['source']['textures'][0]);CASES['equal-face-arrays']=c
for name in ['rotated','reversed']:
    c=deepcopy(CASES['coherent-strip']);s=c['source']
    for i in range(len(s['faces'])):
        for key in ['faces','textures']:
            a=s[key][i];s[key][i]=(a[1:]+a[:1])if name=='rotated'else list(reversed(a))
    CASES[name]=c
c=deepcopy(CASES['coherent-strip']);s=c['source']
s['vertices']=[[0,0,0],[1,0,0],[2,0,0],[2,2,0],[0,2,0],[4,0,0],[4,2,0]]
s['faces']=[[0,1,2,3,4],[2,5,6,3]]
s['textures']=[[[x*.25,-y*.5,(x+y)*.125]for x,y,z in [s['vertices'][v]for v in f]]for f in s['faces']]
CASES['pentagon-and-quad']=c
for name,rows in [('translated',[[1,0,0,7],[0,1,0,11],[0,0,1,17],[0,0,0,1]]),
                  ('mirrored',[[-1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]]),
                  ('scaled',[[2,0,0,0],[0,3,0,0],[0,0,.5,0],[0,0,0,1]]),
                  ('sheared',[[1,.5,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]])]:
    c=deepcopy(CASES['coherent-strip']);c['transform']=rows;CASES[name]=c


def validate(op):
    try:
        ok=(isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_face_custom_holdout'
            and isinstance(op['case'],str)and op['case']in CASES and op['id']==op['case'])
    except TypeError:ok=False
    if not ok:raise ValueError('fixed owned SubD custom-face holdout required')
