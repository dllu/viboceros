"""Public SubD Cap on fixed, independently authored annular cages."""
try:
    from .subd_cap_annuli_cases import CASES, validate
except (ImportError, ValueError):
    from subd_cap_annuli_cases import CASES, validate


def run(op, host, iterations, tolerance):
    validate(op)
    from subd_cap_regions_probe import run_case
    return run_case(CASES[op['case']], host, iterations, tolerance)
