"""SelEdgeRing inputs use the common private component getter driver."""
if __package__:
    from .edge_ring_cases import validate,source
    from .edge_loop_probe import Input
    from .edge_loop_workflow_probe import run_owned
else:
    from edge_ring_cases import validate,source
    from edge_loop_probe import Input
    from edge_loop_workflow_probe import run_owned

def run(op,host,iterations,tolerance):
    validate(op)
    return run_owned(op,host,iterations,tolerance,'SelEdgeRing',lambda op,host:Input(op,host,source(op['scene'])))
