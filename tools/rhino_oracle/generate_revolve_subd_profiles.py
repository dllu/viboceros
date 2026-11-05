"""Freeze independent public friendly-profile and surface-conversion data."""
from .capture_family import capture
from .revolve_subd_profile_probe import request
if __name__=='__main__':
    capture('revolve_subd_profiles',request(),
        ['generate_revolve_subd_profiles.py','revolve_subd_profile_probe.py','revolve_subd_cases.py','revolve_splitting_cases.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveSubDProfiles20261106',timeout=1200)
