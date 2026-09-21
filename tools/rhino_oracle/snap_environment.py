# -*- coding: utf-8 -*-
"""Restored public application settings for owned point-snap calibration."""
from contextlib import contextmanager


@contextmanager
def environment(operation, host, capture_radius=12):
    if type(capture_radius) is not int or not 1 <= capture_radius <= 64:
        raise ValueError("snap capture radius must be an integer from 1 through 64")
    settings = host["Rhino"].ApplicationSettings
    aid, track = settings.ModelAidSettings, settings.SmartTrackSettings
    original_aid, original_track = aid.GetCurrentState(), track.GetCurrentState()
    try:
        aid.GridSnap = aid.Ortho = aid.Planar = False
        aid.Osnap = True
        modes = getattr(settings.OsnapModes, "None")
        names = dict(Point="Point", End="End", Mid="Midpoint", Cen="Center", Quad="Quadrant", Near="Near")
        for name in operation.get("persistent_snaps", []): modes |= getattr(settings.OsnapModes, names[name])
        aid.OsnapModes = modes
        aid.OnlySnapToSelected = False
        aid.OsnapPickboxRadius = capture_radius
        # Rhino 8's available property; do not use Rhino-9-only parallel APIs.
        aid.ProjectSnapToCPlane = False
        aid.SnapToLocked = aid.SnapToOccluded = True
        track.UseSmartTrack = False
        if "snap_to_meshes" in operation:
            if __package__:
                from . import mesh_snap_settings_probe
            else:
                import mesh_snap_settings_probe
            with mesh_snap_settings_probe.environment(operation["snap_to_meshes"], host) as state:
                yield state
        else:
            yield
    finally:
        try:
            aid.UpdateFromState(original_aid)
        finally:
            track.UpdateFromState(original_track)
