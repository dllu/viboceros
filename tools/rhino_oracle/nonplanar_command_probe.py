"""Run fixed public mesh commands in an owned idle document."""
try:
    from .nonplanar_command_cases import CASES,validate
    from .crease_probe import run_scene
except (ImportError,ValueError):
    from nonplanar_command_cases import CASES,validate
    from crease_probe import run_scene


def run(op,host,iterations,tolerance):
    validate(op)
    return run_scene(CASES[op['case']],host,iterations,tolerance)
