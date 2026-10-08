"""Entry point for the narration worker process.

Same application, narration role: it serves only ``/health`` and the stateless
``POST /narration/synthesize``, and it owns its own model instance so it can
synthesize while the main server is busy with a generation. Two processes have
independent MPS contexts, which is safe; two threads sharing one model are not.

The role is set before the app is imported, because ``create_app`` reads it at
import time.
"""

import argparse
import os

os.environ["VOICEBOX_ROLE"] = "narration"

import uvicorn

from . import config, database
from .app import app  # noqa: F401 -- re-export for uvicorn

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="voicebox narration worker")
    parser.add_argument("--host", type=str, default="127.0.0.1")
    parser.add_argument("--port", type=int, default=17494)
    parser.add_argument("--data-dir", type=str, default=None)
    args = parser.parse_args()

    if args.data_dir:
        config.set_data_dir(args.data_dir)

    database.init_db()

    uvicorn.run(
        "backend.narration_main:app",
        host=args.host,
        port=args.port,
        log_level="warning",
        reload=False,
    )
