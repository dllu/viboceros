"""Owned seam-command observations reuse the crease scene and history recorder."""
try:
    from .component_seams_subd_cases import ALL_CASES,validate
    from .crease_probe import run_scene
except (ImportError,ValueError):
    from component_seams_subd_cases import ALL_CASES,validate
    from crease_probe import run_scene


def run(operation,host,iterations,tolerance):
    validate(operation)
    return run_scene(ALL_CASES[operation['case']],host,iterations,tolerance)
