"""Verify local and Git-backed SHA-256 maps in provenance JSON records."""
import argparse
from collections import OrderedDict
from contextlib import ExitStack
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import re
import subprocess
from threading import Lock

ROOT = Path(__file__).resolve().parents[1]
DIGEST = re.compile(r"[0-9a-f]{64}\Z")
REVISION = re.compile(r"[0-9a-f]{40}\Z")
MEASUREMENT_FIELDS = frozenset([
    "measurement_revision", "measurement_record", "measurement_record_sha256",
    "measurement_scope",
])
_GIT_DIGESTS = OrderedDict()
_GIT_DIGESTS_LOCK = Lock()


@lru_cache(maxsize=128)
def _gitlinks(root, revision):
    kind = subprocess.run(["git", "cat-file", "-t", revision],
                          cwd=root, capture_output=True, check=False)
    if kind.returncode or kind.stdout.strip() != b"commit":
        raise ValueError(f"unavailable Git commit {revision}")
    result = subprocess.run(["git", "ls-tree", "-r", "-z", revision],
                            cwd=root, capture_output=True, check=False)
    if result.returncode:
        raise ValueError(f"unavailable Git revision {revision}")
    links = []
    for entry in result.stdout.split(b"\0"):
        if entry.startswith(b"160000 "):
            metadata, path = entry.split(b"\t", 1)
            links.append((path.decode(), metadata.split()[2].decode()))
    return links


def _repository_source(root, revision, path):
    for directory, child_revision in _gitlinks(str(root), revision):
        if path.startswith(directory + "/"):
            child = (root / directory).resolve()
            return _repository_source(child, child_revision, path[len(directory) + 1:])
    return root, revision + ":" + path


class _GitSources:
    """Read immutable blobs in bounded chunks with one Git process per repository."""
    def __init__(self):
        self.stack = ExitStack()
        self.processes = {}

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.stack.close()

    def _process(self, root):
        if root not in self.processes:
            process = self.stack.enter_context(subprocess.Popen(
                ["git", "cat-file", "--batch"], cwd=root, stdin=subprocess.PIPE,
                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL))
            # Closing stdin lets Git finish before Popen's context waits.
            self.stack.callback(process.stdin.close)
            self.processes[root] = process
        return self.processes[root]

    def read(self, root, revision, path, retain=False):
        repository, identity = _repository_source(root, revision, path)
        cache_key = (str(repository), identity)
        with _GIT_DIGESTS_LOCK:
            if not retain and cache_key in _GIT_DIGESTS:
                _GIT_DIGESTS.move_to_end(cache_key)
                return _GIT_DIGESTS[cache_key], None
        process = self._process(repository)
        process.stdin.write(identity.encode() + b"\n")
        process.stdin.flush()
        header = process.stdout.readline().split()
        if len(header) != 3 or header[1] != b"blob":
            raise ValueError(f"unavailable Git source {revision}:{path}")
        remaining = int(header[2])
        digest, chunks = hashlib.sha256(), []
        while remaining:
            chunk = process.stdout.read(min(remaining, 1024 * 1024))
            if not chunk:
                raise ValueError(f"incomplete Git source {revision}:{path}")
            digest.update(chunk)
            if retain:
                chunks.append(chunk)
            remaining -= len(chunk)
        if process.stdout.read(1) != b"\n":
            raise ValueError(f"invalid Git response for {revision}:{path}")
        actual = digest.hexdigest()
        with _GIT_DIGESTS_LOCK:
            _GIT_DIGESTS[cache_key] = actual
            _GIT_DIGESTS.move_to_end(cache_key)
            if len(_GIT_DIGESTS) > 8192:
                _GIT_DIGESTS.popitem(last=False)
        return actual, b"".join(chunks) if retain else None


def _source_path(path):
    if (not isinstance(path, str) or not path or path.startswith("/")
            or any(part in ["", ".", ".."] for part in path.split("/"))
            or any(character in path for character in ["\0", "\n", "\r"])):
        raise ValueError(f"invalid repository source path {path!r}")
    return path


def _maps(record):
    return {name: entries for name, entries in record.items()
            if (name == "sha256" or name.endswith("_sha256"))
            and isinstance(entries, dict)}


def _canonical_payload(record, exclude_maps=False):
    ignored = MEASUREMENT_FIELDS | (set(_maps(record)) if exclude_maps else set())
    # JSON types matter: Python equality otherwise treats False and 0 as equal.
    return json.dumps({key: value for key, value in record.items() if key not in ignored},
                      sort_keys=True, separators=(",", ":"), allow_nan=False)


def _original_measurement(record, root, sources):
    path = _source_path(record["measurement_record"])
    expected = record.get("measurement_record_sha256")
    if not isinstance(expected, str) or not DIGEST.fullmatch(expected):
        raise ValueError("invalid original measurement record digest")
    actual, content = sources.read(root, record["measurement_revision"], path, retain=True)
    if actual != expected:
        raise ValueError("original measurement record SHA-256 mismatch")
    original = json.loads(content)
    if not isinstance(original, dict):
        raise ValueError("original measurement record must be a JSON object")
    if _canonical_payload(record) != _canonical_payload(original):
        raise ValueError("historical measurement differs from its committed original")


def verify(record, root=ROOT):
    """Return the number of checked entries; raise on malformed or changed data.

    Existing maps keep repository-relative file paths. Maps named git_sha256
    or ending in _git_sha256 use '<full commit hash>:<repository path>' keys.
    A full measurement_revision binds ordinary maps to that commit, including
    its recorded submodule revisions. Without it, ordinary maps remain strict
    checks of current files. measurement_record additionally binds the entire
    historical payload and source inventory to its immutable committed JSON.
    """
    root = Path(root).resolve()
    measurement = record.get("measurement_revision")
    if measurement is not None and (not isinstance(measurement, str)
                                    or not REVISION.fullmatch(measurement)):
        raise ValueError("measurement_revision must be a full commit hash")
    if "measurement_record" in record and measurement is None:
        raise ValueError("measurement_record requires measurement_revision")
    checked = 0
    with _GitSources() as sources:
        if measurement:
            _gitlinks(str(root), measurement)
        if "measurement_record" in record:
            _original_measurement(record, root, sources)
            checked += 1
        for name, entries in _maps(record).items():
            from_git = name == "git_sha256" or name.endswith("_git_sha256")
            for path, expected in entries.items():
                if not isinstance(expected, str) or not DIGEST.fullmatch(expected):
                    raise ValueError(f"{name}: invalid digest for {path}")
                revision, filename = measurement, path
                if from_git:
                    revision, separator, filename = path.partition(":")
                    if not separator or not REVISION.fullmatch(revision):
                        raise ValueError(f"{name}: invalid Git reference {path}")
                _source_path(filename)
                if revision:
                    actual, _ = sources.read(root, revision, filename)
                else:
                    with (root / filename).open("rb") as stream:
                        actual = hashlib.file_digest(stream, "sha256").hexdigest()
                if actual != expected:
                    raise ValueError(f"{name}: SHA-256 mismatch for {path}")
                checked += 1
    return checked


def verify_measurement(record, root=ROOT, expand_sources=None):
    """Check current sources or require a versioned record's immutable original."""
    historical = "measurement_revision" in record or "measurement_record" in record
    if historical and "measurement_record" not in record:
        raise ValueError("historical verification requires a committed original record")
    if not historical and expand_sources is not None:
        paths = list(record.get("sha256", {}))
        if set(paths) != set(expand_sources(paths)):
            raise ValueError("current measurement source inventory is incomplete")
    return verify(record, root)


def reconstruct_measurement(record, summarize, *args, root=ROOT, **kwargs):
    """Recheck recorded outcomes without relabeling them as current measurements.

    The original committed record fixes every historical source and data field.
    The current summarizer must still validate all supplied outcomes. Only its
    newly generated provenance maps are replaced by the verified original maps;
    reconstructed geometry, coverage, limits and JSON types must agree exactly.
    """
    verify_measurement(record, root)
    rebuilt = summarize(*args, **kwargs)
    if "measurement_revision" not in record:
        if _canonical_payload(rebuilt) != _canonical_payload(record):
            raise ValueError("current measurements or source bindings differ from reconstruction")
        return rebuilt
    if set(_maps(rebuilt)) != set(_maps(record)):
        raise ValueError("historical report provenance schema changed")
    if _canonical_payload(rebuilt, exclude_maps=True) != _canonical_payload(record, exclude_maps=True):
        raise ValueError("reconstructed measurements differ from the committed original")
    rebuilt.update({name: dict(entries) for name, entries in _maps(record).items()})
    rebuilt.update({key: record[key] for key in MEASUREMENT_FIELDS if key in record})
    return rebuilt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("records", type=Path, nargs="+")
    args = parser.parse_args()
    checked = 0
    for path in args.records:
        try:
            checked += verify(json.loads(path.read_text()))
        except (ValueError, OSError) as error:
            parser.exit(1, f"{path}: {error}\n")
    print(f"Verified {checked} SHA-256 entries in {len(args.records)} records")


if __name__ == "__main__":
    main()
