"""Remembered extraction corner options on independent owned sources."""
if __package__:
    from .subd_extract_probe import run as command
else:
    from subd_extract_probe import run as command


def validate(op):
    if op!={'op':'subd_extract_preferences','id':'corner_option_lifetime'}:
        raise ValueError('fixed extraction preference sequence required')


def run(op,host,iterations,tolerance):
    validate(op);rows=[]
    for scene,corners in [('single',True),('default',False),('single',False),('default',False)]:
        query=dict(op='subd_extract_command',id='-'.join((scene,'remove','corners' if corners else 'smooth')),
                   scene=scene,copy=False,corners=corners)
        value,_=command(query,host,iterations,tolerance);rows.append(value)
    return rows,0
