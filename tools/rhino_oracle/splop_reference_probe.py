"""Fixed public reference-sphere input modes through owned Splop commands."""
import math
try:
    from . import splop_probe as base
except (ImportError,ValueError):
    import splop_probe as base

RECIPES={
 'Radius':['w0,0,0','2'],
 'Diameter':['w0,0,0','_Diameter','4'],
 'Circumference':['w0,0,0','_Circumference',repr(4.*math.pi)],
 'AreaCircle':['w0,0,0','_Area',repr(4.*math.pi)],
 'AreaSphere':['w0,0,0','_Area',repr(16.*math.pi)],
 'OrientationZ':['w0,0,0','_Orientation','w0,0,2','2'],
 'OrientationSpatial':['w0,0,0','_Orientation','w1,2,3','2'],
 'TwoX':['_2Point','w-2,0,0','w2,0,0'],
 'TwoY':['_2Point','w0,-2,0','w0,2,0'],
 'TwoZ':['_2Point','w0,0,-2','w0,0,2'],
 'TwoSpatial':['_2Point','w-1,-2,-3','w1,2,3'],
 'ThreeXY':['_3Point','w2,0,0','w0,2,0','w-2,0,0'],
 'ThreeYZ':['_3Point','w0,2,0','w0,0,2','w0,-2,0'],
 'ThreeRadius':['_3Point','w-1,0,0','w1,0,0','_Radius','2','w0,1,0'],
 'FourAbove':['_4Point','w2,0,0','w0,2,0','w-2,0,0','w0,0,2'],
 'FourBelow':['_4Point','w2,0,0','w0,2,0','w-2,0,0','w0,0,-2'],
 'FourSection':['_4Point','w2,0,1','w0,2,1','w-2,0,1','w0,0,4'],
}


class Forward(object):
    def __init__(self,value):self.value=value
    def __getattr__(self,name):return getattr(self.value,name)


def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','mode','plane'}or op['op']!='splop_reference' or op['mode']not in RECIPES:
        raise ValueError('invalid reference-sphere recipe')
    base.validate(mapped(op))


def mapped(op):
    return dict(op='splop_command',id=op['id'],shape='Cloud',surface='Plane',copy=False,rigid=False,flip=False,grouped=False,angle=0.,radius=4.,plane=op['plane'])


def run(op,host,iterations,tolerance):
    validate(op)
    rhino=Forward(host['Rhino']);app=Forward(rhino.RhinoApp);rhino.RhinoApp=app;state={}
    def execute(macro,echo):
        prefix='_Splop w0,0,0 2 '
        if macro.startswith(prefix):
            macro='_Splop '+' '.join(RECIPES[op['mode']])+' '+macro[len(prefix):]
            state['executed_macro']=macro
        return host['Rhino'].RhinoApp.RunScript(macro,echo)
    app.RunScript=execute;local=dict(host);local['Rhino']=rhino
    value,elapsed=base.run(mapped(op),local,iterations,tolerance)
    value.update(state);return value,elapsed
