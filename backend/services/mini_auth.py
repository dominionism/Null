"""The lock on the mini's endpoints.

`/mini/*` and `/harnesses` run commands on this machine through a harness, so
unlike the rest of the API they are not open to whatever can reach the port. A
caller must be on this machine *and* present a secret that only a process able
to read the data directory can know.

The secret is what stops a web page: a browser will send a request to
127.0.0.1 on behalf of any site that asks, and being on this machine is all
the address check can prove.
"""

from __future__ import annotations

import hmac
import ipaddress
import os
import secrets
from pathlib import Path

from fastapi import HTTPException, Request

from .. import config

_TOKEN_FILE = "mini-token"

_cached: tuple[Path, str] | None = None


def token_path() -> Path:
    return config.get_data_dir() / _TOKEN_FILE


def ensure_token() -> str:
    """The per-install secret, created on first use and readable only by the user."""
    global _cached
    path = token_path()
    if _cached is not None and _cached[0] == path:
        return _cached[1]

    try:
        token = path.read_text().strip()
    except FileNotFoundError:
        token = ""
    if not token:
        token = secrets.token_urlsafe(32)
        path.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w") as handle:
            handle.write(token)
    # The mode passed to os.open is ignored when the file already existed.
    path.chmod(0o600)

    _cached = (path, token)
    return token


def _is_loopback(host: str) -> bool:
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


def require_local_caller(request: Request) -> None:
    """FastAPI dependency: refuse anyone off this machine or without the secret."""
    host = request.client.host if request.client else ""
    if not _is_loopback(host):
        raise HTTPException(status_code=403, detail="The mini's endpoints are only served to this machine.")

    scheme, _, presented = request.headers.get("authorization", "").partition(" ")
    expected = ensure_token()
    if scheme.lower() != "bearer" or not hmac.compare_digest(presented.strip().encode(), expected.encode()):
        raise HTTPException(
            status_code=401,
            detail="Missing or wrong token for the mini's endpoints.",
            headers={"WWW-Authenticate": "Bearer"},
        )
