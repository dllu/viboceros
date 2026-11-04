if __package__:
    from .component_pattern_cases import validate
    from .edge_ring_cases import source
    from .edge_loop_probe import Input
    from .edge_loop_workflow_probe import run_owned
else:
    from component_pattern_cases import validate
    from edge_ring_cases import source
    from edge_loop_probe import Input
    from edge_loop_workflow_probe import run_owned
def run(op,host,iterations,tolerance):
    validate(op)
    return run_owned(op,host,iterations,tolerance,op['command'],lambda op,host:Input(op,host,source(op['scene'])))
