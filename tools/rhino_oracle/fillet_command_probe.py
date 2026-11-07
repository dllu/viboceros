"""Observe the licensed command through public document and curve APIs."""
if __package__:
    from .fillet_command import CASES, validate
else:
    from fillet_command import CASES, validate


def run(op, host, iterations, tolerance):
    validate(op)
    from line_scene_probe import run_case
    def observe(curve):
        return dict(samples=[dict(parameter=curve.Domain.ParameterAt(i / 32.),
                                  point=host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i / 32.))),
                                  tangent=host['_xyz'](curve.TangentAt(curve.Domain.ParameterAt(i / 32.)))) for i in range(33)])
    recipe = CASES[op['case']]
    value, elapsed = run_case(op, host, iterations, tolerance, recipe, 'Fillet', False, observe)
    value['prescribed_case'] = recipe
    return value, elapsed
