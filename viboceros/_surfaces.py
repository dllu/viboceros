"""Tensor-product NURBS surface definitions and native UV samples."""
from __future__ import annotations

from collections.abc import Sequence
from dataclasses import asdict, dataclass
from typing import Literal

from ._types import XYZ

UV = tuple[float, float]
Domain = tuple[tuple[float, float], tuple[float, float]]
RGB = tuple[int, int, int]
Side = Literal["left", "right"]


@dataclass(frozen=True)
class NurbsSurface:
    """A surface control net indexed [u][v], with full knot vectors."""

    degree_u: int
    degree_v: int
    knots_u: Sequence[float]
    knots_v: Sequence[float]
    control_net: Sequence[Sequence[XYZ]]
    weights: Sequence[Sequence[float]] | None = None

    def _payload(self) -> dict:
        return asdict(self)

    @property
    def domain(self) -> Domain:
        nu, nv = len(self.control_net), len(self.control_net[0])
        return (self.knots_u[self.degree_u], self.knots_u[nu]), (self.knots_v[self.degree_v], self.knots_v[nv])


@dataclass(frozen=True)
class SurfacePatch:
    """An exact subdomain of a parent NURBS surface, in its native UV coordinates."""

    domain: Domain | None = None
    name: str | None = None
    color: RGB | None = None

    def _payload(self) -> dict:
        return asdict(self)


@dataclass(frozen=True)
class SurfaceSample:
    uv: UV
    side_u: Side = "right"
    side_v: Side = "right"

    def _payload(self) -> dict:
        return asdict(self)


@dataclass(frozen=True)
class SurfaceJet:
    """Surface point and all first and second partials with respect to native UV."""

    point: XYZ
    du: XYZ
    dv: XYZ
    duu: XYZ
    duv: XYZ
    dvv: XYZ
