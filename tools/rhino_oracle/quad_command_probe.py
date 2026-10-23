"""Execute one fixed native mesh command in an owned idle document."""
try:
    from .quad_command_cases import CASES, validate
    from .crease_probe import run_scene
except (ImportError, ValueError):
    from quad_command_cases import CASES, validate
    from crease_probe import run_scene


def run(op, host, iterations, tolerance):
    validate(op)
    return run_scene(CASES[op['case']], host, iterations, tolerance)
