"""Isolate source knot multiplicity, distinct spans and per-axis Fit capacity."""
import copy
from .capture_family import capture
from .generate_surface_fit_adaptive import bezier, insert
from .generate_surface_knot_reduction import surface


def refined(source, u=(), v=()):
    result=copy.deepcopy(source)
    for axis,values in [('u',u),('v',v)]:
        for value in values:result=insert(result,axis,value)
    return result


def cases():
    original=bezier(True)
    plans=[('base',(),()),('u-mid',(.5,),()),('v-mid',(),(.5,)),
           ('uv-mid',(.5,),(.5,)),('uv-mid-double',(.5,.5),(.5,.5)),
           ('uv-mid-triple',(.5,.5,.5),(.5,.5,.5)),
           ('uv-three-spans',(.25,.5,.75),(.25,.5,.75)),
           ('uv-left-spans',(.125,.25,.5),(.125,.25,.5)),
           ('u-spans-v-multiple',(.25,.5,.75),(.5,.5,.5)),
           ('u-multiple-v-spans',(.5,.5,.5),(.25,.5,.75))]
    for label,u,v in plans:
        yield 'rational-bezier-'+label,refined(original,u,v),3,(.05,.01,.001)
    original=surface('polynomial')
    plans=[('base',(),()),('u-new',(.75,),()),('v-new',(),(.75,)),
           ('uv-new',(.75,),(.75,)),('uv-new-triple',(.75,.75,.75),(.75,.75,.75)),
           ('uv-existing',(.25,),(.25,)),
           ('uv-three-new',(.625,.75,.875),(.625,.75,.875))]
    for label,u,v in plans:
        yield 'polynomial-'+label,refined(original,u,v),3,(.01,)
    yield 'rational-bezier-lower-degree',bezier(True),2,(.01,)


def request():
    operations=[]
    for name,source,degree,limits in cases():
        for limit in limits:
            operations.append(dict(op='surface_fit',id='fit-capacity-'+name+'-'+str(limit),surface=source,
                                   degree_u=degree,degree_v=degree,fit_tolerance=limit))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__=='__main__':
    capture('surface_fit_capacity',request(),['generate_surface_fit_capacity.py',
        'generate_surface_fit_adaptive.py','generate_surface_knot_reduction.py',
        'generate_surface_knot_reduction_holdouts.py','surface_fit_probe.py',
        'structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleSurfaceFitCapacity20261109',timeout=1200)
