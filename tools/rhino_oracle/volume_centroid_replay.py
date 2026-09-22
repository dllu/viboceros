"""Separate actual VolumeCentroid replay from raw API diagnostics."""
from .area_centroid_replay import main


if __name__ == "__main__": raise SystemExit(main(measure="volume"))
