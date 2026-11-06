"""Fixed whole-file cancellation and completed Lines integration inputs."""
MACROS={
 'macro_empty':dict(actions=[],macro='! _Lines _Enter _Point w9,9,9\n'),
 'macro_anchor':dict(actions=[],macro='! _Lines w0,0,0 _Enter _Point w9,9,9\n'),
 'macro_live':dict(actions=[],macro='! _Lines w0,0,0 w4,0,0 _Enter _Point w9,9,9\n'),
 'macro_closed':dict(actions=[],macro='! _Lines w0,0,0 w4,0,0 w4,3,1 _Close _Point w9,9,9\n'),
}
