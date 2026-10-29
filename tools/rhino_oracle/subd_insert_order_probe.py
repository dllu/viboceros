"""Replay a fixed independent insertion recipe through the acknowledged getter."""
try:
    from .subd_insert_order_cases import CASES,validate
    from .subd_insert_custom_probe import workflow
except (ImportError,ValueError):
    from subd_insert_order_cases import CASES,validate
    from subd_insert_custom_probe import workflow


def run(op,host,iterations,tolerance):
    validate(op)
    return workflow(op,CASES[op['case']],host,iterations,tolerance)
