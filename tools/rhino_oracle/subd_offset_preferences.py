"""Fresh defaults and remembered OffsetSubD options, including cancellation."""
if __package__:
    from .subd_offset_command_probe import run_case
else:
    from subd_offset_command_probe import run_case


def cases():
    base=dict(scene='quad',distance=.75,solid=False,both_sides=False,delete_input=False,flip=False,preselect=True)
    return [dict(base,defaults=True),dict(base,distance=1.25,solid=True,both_sides=True,delete_input=True),
            dict(base,defaults=True),base,dict(base,defaults=True),
            dict(base,distance=2.5,solid=True,both_sides=False,delete_input=True,cancel=True),dict(base,defaults=True)]


def validate(op):
    if op!={'op':'subd_offset_preferences','id':'option_lifetime'}:raise ValueError('fixed SubD offset preference sequence required')


def run(op,host,iterations,tolerance):
    validate(op);rows=[]
    for case in cases():
        result,_=run_case(case,host,iterations,tolerance);rows.append(dict(input=case,value=result))
    return rows,0
