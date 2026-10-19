"""Use the public ToNurbsCurve conversion before displaying Weight grips."""


def validate(op):
    if __package__:
        from .grip_weight_probe import validate as base
    else:
        from grip_weight_probe import validate as base
    mapped=dict(op,op='grip_weight');base(mapped)
    if op['op']!='grip_weight_nurbs' or any(s['kind']not in('circle','arc')for s in op['sources']):
        raise ValueError('invalid NURBS Weight source')


def run(op,host):
    validate(op)
    from grip_weight_probe import run as base
    from delete_grips_probe import create_source
    def source(kind,environment):
        original=create_source(kind,environment)
        try:return original.ToNurbsCurve()
        finally:original.Dispose()
    return base(dict(op,op='grip_weight'),dict(host,_weight_source=source))
