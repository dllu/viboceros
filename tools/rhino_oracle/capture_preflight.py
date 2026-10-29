"""Ordered request policy and interaction setup, before any Rhino launch."""
from . import capture_preflight_geometry as geometry
from . import capture_preflight_construction as construction
from . import capture_preflight_surfaces as surfaces
from . import capture_preflight_components as components
from . import capture_preflight_curves as curves
from . import capture_preflight_modeling as modeling
from . import capture_preflight_picking as picking

# Capture provenance must bind host helper bodies as well as the client entry.
CLIENT_SOURCE_FILES = (
    'client.py', 'oracle_errors.py', 'capture_preflight.py',
    'capture_preflight_geometry.py', 'capture_preflight_construction.py',
    'capture_preflight_surfaces.py', 'capture_preflight_components.py',
    'capture_preflight_curves.py', 'capture_preflight_modeling.py',
    'capture_preflight_picking.py', 'capture_preflight_common.py',
)


def validate_sources(client, request):
    geometry.validate_sources(client, request)


def prepare_capture(client, request, worker_source):
    interaction = geometry.prepare(client, request)
    interaction = construction.prepare(client, request, interaction)
    interaction = surfaces.prepare(client, request, interaction)
    interaction = components.prepare(client, request, interaction)
    interaction = curves.prepare(client, request, interaction)
    interaction = modeling.prepare(client, request, interaction)
    return picking.prepare(client, request, interaction, worker_source)
