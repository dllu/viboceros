"""Multiple source command order, group copies and selection modes."""
if __package__:
    from .subd_offset_command_probe import run_case
else:
    from subd_offset_command_probe import run_case


def cases():
    return [dict(op='subd_offset_batch',id='-'.join(('solid' if solid else 'skin','both' if both else 'one','pre' if pre else 'post')),
                 solid=solid,both_sides=both,preselect=pre) for solid in (False,True)for both in (False,True)for pre in (False,True)]


def validate(op):
    if (not isinstance(op,dict) or any(type(op.get(key))is not bool for key in ('solid','both_sides','preselect')) or not any(op==row for row in cases())):
        raise ValueError('fixed SubD offset batch required')


def run(op,host,iterations,tolerance):
    validate(op)
    return run_case(dict(op,scene='quad',many=['quad','closed'],distance=.75,delete_input=False,flip=False),host,iterations,tolerance)
