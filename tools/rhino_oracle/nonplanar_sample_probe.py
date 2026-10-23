"""Public scripted threshold getters with acknowledged physical face picks."""
try:
    from .nonplanar_sample_cases import CASES,validate
    from .crease_probe import run_scene
except (ImportError,ValueError):
    from nonplanar_sample_cases import CASES,validate
    from crease_probe import run_scene


def run(op,host,iterations,tolerance):
    validate(op)
    return run_scene(CASES[op['case']],host,iterations,tolerance)
