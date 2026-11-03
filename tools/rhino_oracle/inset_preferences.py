"""Owned grouped/single inset preference clocks and cancellation."""
if __package__:
    from .inset_probe import run_scene
else:
    from inset_probe import run_scene
STEPS=[dict(mode=None,distance=.3),dict(mode='Single',distance=.3,cancel=True),dict(mode=None,distance=.3),dict(mode='Group',distance=.4),dict(mode='Single',distance=.4),dict(mode='Group',distance=.4,cancel=True),dict(mode=None,distance=0.)]
query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=[dict(op='inset_preferences',id='inset-options')])
def validate(op):
    if op!={'op':'inset_preferences','id':'inset-options'}:raise ValueError('fixed owned inset preference sequence required')
def run(op,host,iterations,tolerance):
    validate(op);rows=[]
    for step in STEPS:
        value,_=run_scene(dict(step,op='inset_preferences',target='bent_both'),host,iterations,tolerance,'subd');rows.append(dict(input=step,value=value))
    return rows,0
