"""Deliver clicks and Escape only to the verified private OffsetSrf window."""
from .chamfer_point_picker import ChamferPointPicker

class OffsetSurfacePointPicker(ChamferPointPicker):
    key_prefix='@offset-srf-escape:'
