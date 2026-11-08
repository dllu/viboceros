"""Click surface edges and cancel only in the verified private Rhino window."""
from .chamfer_point_picker import ChamferPointPicker


class MatchSurfacePointPicker(ChamferPointPicker):
    key_prefix='@match-srf-escape:'
