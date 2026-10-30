"""Verify retained producer bytes and historical measurement payloads."""
import gzip
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ANNOTATIONS = {'qualified_revision', 'current_source_receipt', 'source_hashes_scope'}


def frozen(record, name):
    digest = record['provenance']['sources'][name]
    data = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError('corrupt frozen producer')
    return data


def measurement_digest(record, annotations=ANNOTATIONS):
    payload = {k: v for k, v in record.items() if k not in annotations}
    return hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
