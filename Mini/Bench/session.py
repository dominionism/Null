"""A run's numbers, read from the conversation file the harness wrote for it.

The harness writes one JSON object a line: a `session` line first, then a
`message` line for each turn of the person, the model and the tools. From it:
how long the run took, how many steps (tool calls) it made and how many of them
failed, how many turns the model took, how many pictures it was handed, the
tokens it used, and the last thing it said, which is the answer a checker reads.
Only the bench's own runs are read here, from a folder of the run's own.
"""

import datetime
import glob
import json
import os


def _when(value):
    if isinstance(value, (int, float)):
        return value / 1000 if value > 1e11 else float(value)
    if isinstance(value, str):
        try:
            return datetime.datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()
        except ValueError:
            return None
    return None


def files(folder):
    """The conversation files under a run's session folder, the newest last."""
    found = [p for p in glob.glob(os.path.join(folder, "**", "*.jsonl"), recursive=True) if not os.path.basename(p).startswith(".")]
    return sorted(found, key=os.path.getmtime)


def read(path):
    """The numbers of one conversation file."""
    entries = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    entries.append(json.loads(line))
                except ValueError:
                    pass
    return numbers(entries)


def numbers(entries):
    messages = [e["message"] for e in entries if e.get("type") == "message" and isinstance(e.get("message"), dict)]
    times = [t for t in (_when(m.get("timestamp")) for m in messages) if t]
    first_user = next((_when(m.get("timestamp")) for m in messages if m.get("role") == "user"), None)
    assistant = [m for m in messages if m.get("role") == "assistant"]
    calls = [part for m in assistant for part in (m.get("content") or []) if isinstance(part, dict) and part.get("type") == "toolCall"]
    results = [m for m in messages if m.get("role") == "toolResult"]
    pictures = sum(1 for m in results for part in (m.get("content") or []) if isinstance(part, dict) and part.get("type") == "image")
    tools = {}
    for c in calls:
        tools[c.get("name", "?")] = tools.get(c.get("name", "?"), 0) + 1
    usage = {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0, "cost": 0.0}
    for m in assistant:
        u = m.get("usage") or {}
        usage["input"] += u.get("input", 0) or 0
        usage["output"] += u.get("output", 0) or 0
        usage["cache_read"] += u.get("cacheRead", 0) or 0
        usage["cache_write"] += u.get("cacheWrite", 0) or 0
        cost = u.get("cost")
        usage["cost"] += (cost.get("total", 0) if isinstance(cost, dict) else cost or 0) or 0
    last = next((m for m in reversed(assistant) if any(isinstance(p, dict) and p.get("type") == "text" and p.get("text", "").strip() for p in m.get("content") or [])), None)
    answer = "\n".join(p["text"] for p in (last.get("content") or []) if isinstance(p, dict) and p.get("type") == "text") if last else ""
    model = next((f"{m.get('provider')}/{m.get('model')}" for m in reversed(assistant) if m.get("model")), None)
    return {
        "seconds": round(max(times) - (first_user or min(times)), 1) if times else None,
        "steps": len(calls),
        "failed_steps": sum(1 for m in results if m.get("isError")),
        "turns": len(assistant),
        "pictures": pictures,
        "tools": tools,
        "tokens": usage,
        "model": model,
        "stop": assistant[-1].get("stopReason") if assistant else None,
        "answer": answer.strip(),
    }
