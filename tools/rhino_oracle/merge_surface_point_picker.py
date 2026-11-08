"""Deliver MergeSrf Escape only to the verified private Rhino window."""
from .chamfer_point_picker import ChamferPointPicker


class MergeSurfacePointPicker(ChamferPointPicker):
    key_prefix = '@merge-srf-escape:'
