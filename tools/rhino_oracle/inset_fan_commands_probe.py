"""Owned command modes exposing crossed-cap curves and source subdivisions."""
if __package__:
    from . import inset_fan_interior_probe as interior
    from . import inset_fan_basis_probe as basis
    from .inset_outputs_probe import run_with_definitions
else:
    import inset_fan_interior_probe as interior
    import inset_fan_basis_probe as basis
    from inset_outputs_probe import run_with_definitions


def cases():
    sources=[interior.cases()[0],basis.cases()[-1]]
    return [dict(op,id=op['id']+'_'+mode,op='inset_fan_commands',
                 split=mode!='curves',join_outputs=mode=='joined')
            for op in sources for mode in ('curves','pieces','joined')]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()
            or type(op.get('distance')) is not float
            or any(type(op.get(k)) is not bool for k in
                   ('crease_corners','loose','ignore_seams','split','join_outputs'))
            or ('degree' in op and (type(op['degree']) is not int
                                   or type(op.get('radius_x')) is not float
                                   or type(op.get('subdivide')) is not bool))):
        raise ValueError('invalid owned crossed-cap Inset command recipe')


def run(op,host,iterations,tolerance):
    validate(op)
    b=(basis.source if op['scene'].startswith('hole_') else interior.source)(op,host,tolerance)
    return run_with_definitions(op,host,iterations,tolerance,b)
