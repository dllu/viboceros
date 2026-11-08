"""Measure shifted domains, rational clocks, curved edges and short multispans."""
from .capture_family import capture
from .match_surface_cases import FINAL,request


if __name__=='__main__':
    capture('match_surface_final',request(list(FINAL)),[
        'generate_match_surface_final.py','match_surface_cases.py','match_surface_sdk.py',
        'structural_sdk.py','rhino_worker.py','client.py','brep_join_probe.py','cap_probe.py',
    ],'VibocerosOracleMatchSurfaceFinal20261108',timeout=1200)
