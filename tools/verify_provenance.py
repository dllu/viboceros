"""Verify local and Git-backed SHA-256 maps in provenance JSON records."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIGEST = re.compile(r"[0-9a-f]{64}\Z")
REVISION = re.compile(r"[0-9a-f]{40}\Z")


def verify(record, root=ROOT):
    """Return the number of checked entries; raise on malformed or changed data.

    Existing maps keep repository-relative file paths. Maps named git_sha256
    or ending in _git_sha256 use '<full commit hash>:<repository path>' keys.
    Git history retains the original bytes after a source file changes or is
    deleted, without placing additional implementation copies in the worktree.
    """
    checked = 0
    for name, entries in record.items():
        if not (name == "sha256" or name.endswith("_sha256")) or not isinstance(entries, dict):
            continue
        from_git = name == "git_sha256" or name.endswith("_git_sha256")
        for path, expected in entries.items():
            if not isinstance(expected, str) or not DIGEST.fullmatch(expected):
                raise ValueError(f"{name}: invalid digest for {path}")
            if from_git:
                revision, separator, filename = path.partition(":")
                if not separator or not REVISION.fullmatch(revision) or not filename:
                    raise ValueError(f"{name}: invalid Git reference {path}")
                result = subprocess.run(
                    ["git", "cat-file", "blob", f"{revision}:{filename}"],
                    cwd=root, capture_output=True, check=False,
                )
                if result.returncode:
                    raise ValueError(f"{name}: unavailable Git source {path}")
                data = result.stdout
            else:
                data = (root / path).read_bytes()
            actual = hashlib.sha256(data).hexdigest()
            if actual != expected:
                raise ValueError(f"{name}: SHA-256 mismatch for {path}")
            checked += 1
    return checked


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
