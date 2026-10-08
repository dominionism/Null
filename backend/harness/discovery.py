"""Find a harness's CLI on this machine.

The server runs under launchd with a minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin`),
so a harness the user runs every day from a terminal is usually *not* on it.
Look where harnesses install themselves before giving up.
"""

from __future__ import annotations

import os
import shutil
from pathlib import Path

#: Where harness CLIs install themselves, in the order tried.
_INSTALL_DIRS = (
    "~/.omp/bin",
    "~/.opencode/bin",
    "~/.local/bin",
    "/opt/homebrew/bin",
    "/usr/local/bin",
)


def find_binary(name: str, *, override: str | None = None) -> str | None:
    """Absolute path to the CLI `name`, or None when it isn't installed.

    `override` is a path the user set by hand. It wins when it points at
    something runnable; otherwise it is ignored and the search goes on.
    """
    if override:
        path = Path(override).expanduser()
        if _runnable(path):
            return str(path)
    found = shutil.which(name)
    if found:
        return found
    for directory in _INSTALL_DIRS:
        path = Path(directory).expanduser() / name
        if _runnable(path):
            return str(path)
    return None


def _runnable(path: Path) -> bool:
    return path.is_file() and os.access(path, os.X_OK)
