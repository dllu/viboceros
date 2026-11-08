"""Windowless rational patch offsets and adjacent command/UI regressions."""
from .check_offset_srf_trims import ROOT,commands as trim_commands
PROOF=ROOT/'docs/offset-srf-rational-proof'

def commands():
    for name,command in trim_commands():
        # Geometry and OpenNURBS sources must match the qualified baseline.
        # The report verifies their Git trees; do not relabel its old test log.
        if name=='geometry':continue
        if name=='python':
            command=[*command,'tools.rhino_oracle.test_certificate_norm','tools.rhino_oracle.test_offset_srf_rational']
        yield name,command
        if name=='native-sheet-export':
            yield 'native-rational-fit',['cargo','test','-p','viboceros-smlib','--features','native','--test','surface_offset']

if __name__=='__main__':
    from . import check_offset_srf_trims as runner
    runner.PROOF=PROOF;runner.commands=commands;runner.main()
