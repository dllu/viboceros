"""SelFaceLoop observes native edge seeds and accepted face components."""
if __package__:
    from .face_loop_cases import validate
    from .edge_ring_cases import source
    from .edge_loop_probe import Input
    from .edge_loop_workflow_probe import run_owned
else:
    from face_loop_cases import validate
    from edge_ring_cases import source
    from edge_loop_probe import Input
    from edge_loop_workflow_probe import run_owned

def run(op,host,iterations,tolerance):
    validate(op)
    return run_owned(op,host,iterations,tolerance,'SelFaceLoop',lambda op,host:Input(op,host,source(op['scene'])))
