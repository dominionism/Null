#!/usr/bin/env python3
"""PROTOTYPE — throwaway. Delete when Context/Plans/NullMini.md item 4 lands.

Question (NullMini.md, item 1): can another program drive Oh-my-pi well enough
to be the brain of a small text box? Specifically, over `omp acp`:

  - how long until the first text arrives, and how finely does it stream
  - how tool use and permission requests are reported
  - how a model is chosen for a session
  - how fast a reply can be cancelled
  - whether Null's MCP server can be attached to one session
  - whether all of that still works from the minimal environment launchd gives
    the Null server

Run:  python3 scripts/prototype-omp-spike.py            (every scenario)
      python3 scripts/prototype-omp-spike.py explore     (handshake + session only, no prompts)
      python3 scripts/prototype-omp-spike.py ask "..."   (one prompt, streamed to the terminal)

Stdlib only. Speaks raw JSON-RPC over stdio; the real adapter will use the
agent-client-protocol SDK instead.
"""

import json
import os
import queue
import subprocess
import sys
import tempfile
import threading
import time

OMP_CANDIDATES = ["~/.omp/bin/omp", "~/.local/bin/omp", "/opt/homebrew/bin/omp", "/usr/local/bin/omp"]
PROMPT_TIMEOUT_S = 180


def find_omp():
    override = os.environ.get("OMP_BIN")
    if override:
        return override
    for candidate in OMP_CANDIDATES:
        path = os.path.expanduser(candidate)
        if os.path.isfile(path) and os.access(path, os.X_OK):
            return path
    sys.exit("omp not found; set OMP_BIN")


def short(value, limit=220):
    text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)
    return text if len(text) <= limit else text[:limit] + f"… (+{len(text) - limit})"


class AcpClient:
    """One `omp acp` child process and the JSON-RPC conversation with it."""

    def __init__(self, omp, cwd, permission_choice="allow", extra_args=()):
        self.cwd = cwd
        self.permission_choice = permission_choice
        self.inbox = queue.Queue()
        self.next_id = 1
        self.updates = []  # (elapsed_s_since_prompt, update dict)
        self.permission_requests = []
        self.other_requests = []
        self.prompt_started = None
        self.echo_text = False
        self.proc = subprocess.Popen(
            [omp, *extra_args, "acp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=cwd,
        )
        self.stderr_tail = []
        threading.Thread(target=self._read_stdout, daemon=True).start()
        threading.Thread(target=self._read_stderr, daemon=True).start()

    def _read_stdout(self):
        for raw in self.proc.stdout:
            raw = raw.strip()
            if not raw:
                continue
            try:
                self.inbox.put(json.loads(raw))
            except ValueError:
                self.inbox.put({"_unparsed": raw.decode("utf-8", "replace")})
        self.inbox.put({"_eof": True})

    def _read_stderr(self):
        for raw in self.proc.stderr:
            self.stderr_tail.append(raw.decode("utf-8", "replace").rstrip())
            del self.stderr_tail[:-20]

    def _send(self, message):
        self.proc.stdin.write((json.dumps(message) + "\n").encode())
        self.proc.stdin.flush()

    def notify(self, method, params):
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method, params, timeout=60, on_tick=None):
        """Send a request and pump the inbox until its response arrives."""
        request_id = self.next_id
        self.next_id += 1
        self._send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                message = self.inbox.get(timeout=0.05)
            except queue.Empty:
                if on_tick:
                    on_tick()
                continue
            if message.get("_eof"):
                return {"error": {"message": "omp exited", "stderr": self.stderr_tail[-5:]}}
            if "_unparsed" in message:
                continue
            if message.get("id") == request_id and "method" not in message:
                return message
            self._dispatch(message)
            if on_tick:
                on_tick()
        return {"error": {"message": f"timeout after {timeout}s waiting for {method}"}}

    def _dispatch(self, message):
        method = message.get("method")
        params = message.get("params") or {}
        if method == "session/update":
            elapsed = time.time() - self.prompt_started if self.prompt_started else 0.0
            update = params.get("update") or {}
            self.updates.append((elapsed, update))
            if self.echo_text and update.get("sessionUpdate") == "agent_message_chunk":
                sys.stdout.write((update.get("content") or {}).get("text", ""))
                sys.stdout.flush()
            return
        if method == "session/request_permission":
            self.permission_requests.append(params)
            options = params.get("options") or []
            wanted = "allow" if self.permission_choice == "allow" else "reject"
            chosen = next((o for o in options if str(o.get("kind", "")).startswith(wanted)), None)
            outcome = {"outcome": "selected", "optionId": chosen["optionId"]} if chosen else {"outcome": "cancelled"}
            self._send({"jsonrpc": "2.0", "id": message["id"], "result": {"outcome": outcome}})
            return
        if "id" in message and method:
            # A request this prototype does not implement (fs/*, terminal/*, …).
            self.other_requests.append(method)
            self._send({"jsonrpc": "2.0", "id": message["id"], "error": {"code": -32601, "message": "not supported"}})

    def prompt(self, session_id, text, cancel_after_first_text_s=None):
        """Send one prompt; return timings and what came back."""
        self.updates = []
        self.permission_requests = []
        self.prompt_started = time.time()
        cancel_sent = {"at": None}

        def maybe_cancel():
            if cancel_after_first_text_s is None or cancel_sent["at"] is not None:
                return
            first = next((t for t, u in self.updates if u.get("sessionUpdate") == "agent_message_chunk"), None)
            if first is not None and time.time() - self.prompt_started >= first + cancel_after_first_text_s:
                self.notify("session/cancel", {"sessionId": session_id})
                cancel_sent["at"] = time.time()

        response = self.request(
            "session/prompt",
            {"sessionId": session_id, "prompt": [{"type": "text", "text": text}]},
            timeout=PROMPT_TIMEOUT_S,
            on_tick=maybe_cancel,
        )
        finished = time.time()
        total = finished - self.prompt_started
        kinds = {}
        for _, update in self.updates:
            kind = update.get("sessionUpdate", "?")
            kinds[kind] = kinds.get(kind, 0) + 1
        text_chunks = [(t, (u.get("content") or {}).get("text", "")) for t, u in self.updates
                       if u.get("sessionUpdate") == "agent_message_chunk"]
        return {
            "response": response,
            "total_s": round(total, 2),
            "first_update_s": round(self.updates[0][0], 2) if self.updates else None,
            "first_text_s": round(text_chunks[0][0], 2) if text_chunks else None,
            "text_chunks": len(text_chunks),
            "text": "".join(chunk for _, chunk in text_chunks),
            "avg_chunk_chars": round(sum(len(c) for _, c in text_chunks) / len(text_chunks), 1) if text_chunks else 0,
            "update_kinds": kinds,
            "tool_updates": [u for _, u in self.updates if u.get("sessionUpdate", "").startswith("tool_call")],
            "usage_updates": [u for _, u in self.updates if u.get("sessionUpdate") == "usage_update"],
            "permission_requests": list(self.permission_requests),
            "cancel_to_return_s": round(finished - cancel_sent["at"], 2) if cancel_sent["at"] else None,
        }

    def close(self):
        try:
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()


def heading(title):
    print(f"\n\033[1m== {title}\033[0m")


def show_prompt_result(result, show_text=True):
    response = result["response"]
    print(f"  stop: {short(response.get('result') or response.get('error'))}")
    print(f"  first update {result['first_update_s']} s | first text {result['first_text_s']} s | total {result['total_s']} s")
    print(f"  text chunks: {result['text_chunks']} (avg {result['avg_chunk_chars']} chars) | update kinds: {result['update_kinds']}")
    if show_text:
        print(f"  text: {short(result['text'].strip(), 300)}")
    for update in result["tool_updates"][:6]:
        print(f"  tool: {short(update, 260)}")
    for request in result["permission_requests"][:3]:
        print(f"  permission request: {short(request, 320)}")
    for update in result["usage_updates"][-1:]:
        print(f"  usage: {short(update, 260)}")
    if result["cancel_to_return_s"] is not None:
        print(f"  cancel → prompt returned in {result['cancel_to_return_s']} s")


def open_session(client, mcp_servers=None):
    started = time.time()
    init = client.request("initialize", {
        "protocolVersion": 1,
        "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}, "terminal": False},
        "clientInfo": {"name": "null-mini-spike", "version": "0"},
    })
    init_s = time.time() - started
    started = time.time()
    new = client.request("session/new", {"cwd": client.cwd, "mcpServers": mcp_servers or []})
    new_s = time.time() - started
    return init, round(init_s, 2), new, round(new_s, 2)


def describe_session(new):
    result = new.get("result")
    if not result:
        print(f"  session/new failed: {short(new.get('error'), 400)}")
        return None
    print(f"  sessionId: {result.get('sessionId')}")
    print(f"  result keys: {sorted(result.keys())}")
    for key in ("modes", "models"):
        if key in result:
            print(f"  {key}: {short(result[key], 500)}")
    for option in result.get("configOptions") or []:
        choices = option.get("options") or []
        flat = []
        for choice in choices:
            if "options" in choice:  # grouped
                flat.extend(choice["options"])
            else:
                flat.append(choice)
        print(f"  configOption id={option.get('id')} category={option.get('category')} "
              f"current={option.get('currentValue')} choices={len(flat)}")
        print(f"    e.g. {short([c.get('value') for c in flat[:12]], 400)}")
    return result.get("sessionId")


def set_config(client, session_id, config_id, value):
    return client.request("session/set_config_option", {"sessionId": session_id, "configId": config_id, "value": value})


def model_choices(new):
    for option in (new.get("result") or {}).get("configOptions") or []:
        if option.get("id") == "model":
            flat = []
            for choice in option.get("options") or []:
                flat.extend(choice["options"] if "options" in choice else [choice])
            return [c.get("value") for c in flat], option.get("currentValue")
    return [], None


def extras(omp, scratch):
    """Follow-up checks: permission options, approval flags, refusal, thinking, provider switch, reload."""
    tool_prompt = "Run the shell command `echo null-mini-spike` and reply with only its output."

    heading("permission request in full (default approval mode)")
    client = AcpClient(omp, scratch)
    try:
        _, _, new, _ = open_session(client)
        session_id = new["result"]["sessionId"]
        models, current = model_choices(new)
        providers = sorted({m.split("/", 1)[0] for m in models})
        print(f"  current model: {current} | providers offered: {providers}")
        result = client.prompt(session_id, tool_prompt)
        for request in result["permission_requests"]:
            print(f"  options: {json.dumps(request.get('options'))}")
        print(f"  permission requests: {len(result['permission_requests'])} | text: {short(result['text'].strip())}")

        heading("thinking = off (same session, warm)")
        print(f"  set: {short(set_config(client, session_id, 'thinking', 'off'), 160)}")
        for word in ("one", "two", "three"):
            r = client.prompt(session_id, f"Reply with exactly one word: {word}")
            print(f"  first text {r['first_text_s']} s | total {r['total_s']} s | {short(r['text'].strip(), 40)}")

        other = os.environ.get("SPIKE_OTHER_MODEL")
        if other:
            heading(f"switch provider mid-conversation → {other}")
            print(f"  set: {short(set_config(client, session_id, 'model', other), 200)}")
            r = client.prompt(session_id, "What were the three single words I just asked you to reply with? One line.")
            show_prompt_result(r)
        saved_session = session_id
    finally:
        client.close()

    heading("refusing a permission request")
    client = AcpClient(omp, scratch, permission_choice="reject")
    try:
        _, _, new, _ = open_session(client)
        result = client.prompt(new["result"]["sessionId"], tool_prompt)
        print(f"  permission requests: {len(result['permission_requests'])} | stop: {short(result['response'].get('result'))}")
        print(f"  tool statuses: {[u.get('status') for u in result['tool_updates']]}")
        print(f"  text: {short(result['text'].strip(), 240)}")
    finally:
        client.close()

    for flags in (("--approval-mode", "yolo"), ("--approval-mode", "always-ask")):
        heading(f"omp {' '.join(flags)} acp")
        client = AcpClient(omp, scratch, extra_args=flags)
        try:
            _, _, new, _ = open_session(client)
            if "result" not in new:
                print(f"  failed: {short(new.get('error'), 300)} | stderr: {client.stderr_tail[-3:]}")
                continue
            result = client.prompt(new["result"]["sessionId"], tool_prompt)
            print(f"  permission requests: {len(result['permission_requests'])} | text: {short(result['text'].strip(), 80)}")
        finally:
            client.close()

    heading("reload the earlier session in a new process (no model call)")
    client = AcpClient(omp, scratch)
    try:
        client.request("initialize", {"protocolVersion": 1, "clientCapabilities": {}, "clientInfo": {"name": "null-mini-spike", "version": "0"}})
        client.prompt_started = time.time()
        loaded = client.request("session/load", {"sessionId": saved_session, "cwd": scratch, "mcpServers": []}, timeout=30)
        kinds = {}
        for _, update in client.updates:
            kinds[update.get("sessionUpdate", "?")] = kinds.get(update.get("sessionUpdate", "?"), 0) + 1
        print(f"  session/load → {short(loaded.get('error') or sorted((loaded.get('result') or {}).keys()), 200)}")
        print(f"  replayed updates: {kinds}")
        listed = client.request("session/list", {"cwd": scratch}, timeout=20)
        sessions = (listed.get("result") or {}).get("sessions") or []
        print(f"  session/list for this cwd: {len(sessions)} session(s); first: {short(sessions[0], 240) if sessions else listed.get('error')}")
    finally:
        client.close()


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "all"
    omp = find_omp()
    scratch = tempfile.mkdtemp(prefix="null-mini-spike-")
    print(f"omp: {omp}\nscratch cwd: {scratch}\nPATH: {os.environ.get('PATH', '')[:120]}")

    if mode == "extras":
        extras(omp, scratch)
        return

    if mode == "switch":
        # One conversation, two providers: does context survive a model change mid-session?
        target = sys.argv[2] if len(sys.argv) > 2 else "openai-codex/gpt-5.6-luna"
        client = AcpClient(omp, scratch)
        try:
            init, init_s, new, new_s = open_session(client)
            session_id = new["result"]["sessionId"]
            _, current = model_choices(new)
            heading(f"turn 1 on {current}")
            show_prompt_result(client.prompt(session_id, "Remember this word: maple. Reply with exactly: OK"))
            heading(f"switch model → {target}")
            switched = set_config(client, session_id, "model", target)
            print(f"  {short(switched.get('error') or 'accepted', 300)}")
            heading(f"turn 2 on {target}")
            show_prompt_result(client.prompt(session_id, "Which word did I ask you to remember? Reply with that one word."))
        finally:
            if client.stderr_tail:
                print("omp stderr tail:\n  " + "\n  ".join(client.stderr_tail[-5:]))
            client.close()
        return

    client = AcpClient(omp, scratch)
    try:
        heading("handshake + new session")
        init, init_s, new, new_s = open_session(client)
        print(f"  initialize {init_s} s → {short(init.get('result') or init.get('error'), 300)}")
        print(f"  session/new {new_s} s")
        session_id = describe_session(new)
        if not session_id or mode == "explore":
            if mode == "explore":
                print(f"\n  raw session/new result:\n  {short(new.get('result'), 2500)}")
            return

        model_config = os.environ.get("SPIKE_MODEL")  # "configId=value"
        if model_config:
            config_id, value = model_config.split("=", 1)
            heading(f"select model: {config_id} = {value}")
            print(f"  {short(set_config(client, session_id, config_id, value), 400)}")

        if mode == "ask":
            client.echo_text = True
            result = client.prompt(session_id, " ".join(sys.argv[2:]) or "Say hello in five words.")
            client.echo_text = False
            print()
            show_prompt_result(result, show_text=False)
            return

        heading("first prompt (cold session)")
        show_prompt_result(client.prompt(session_id, "Reply with exactly one word: pong"))

        heading("second prompt (same session, warm)")
        show_prompt_result(client.prompt(session_id, "Reply with exactly one word: ping"))

        heading("streaming granularity (a longer reply)")
        show_prompt_result(client.prompt(session_id, "In about 80 words, explain what a CLI is. Plain prose, no lists."))

        heading("tool use (a harmless shell command)")
        show_prompt_result(client.prompt(
            session_id, "Run the shell command `echo null-mini-spike` and reply with only its output."))

        heading("cancel one second after the first text")
        show_prompt_result(client.prompt(
            session_id, "Count from 1 to 400, one number per line, no other text.", cancel_after_first_text_s=1.0),
            show_text=False)

        heading("the session still works after a cancel")
        show_prompt_result(client.prompt(session_id, "Reply with exactly one word: alive"))

        heading("error shape: prompt to a session that does not exist")
        print(f"  {short(client.request('session/prompt', {'sessionId': 'does-not-exist', 'prompt': [{'type': 'text', 'text': 'hi'}]}, timeout=20), 400)}")
    finally:
        if client.other_requests:
            print(f"\nagent→client requests this prototype refused: {sorted(set(client.other_requests))}")
        if client.proc.poll() not in (None, 0) and client.stderr_tail:
            print("omp stderr tail:\n  " + "\n  ".join(client.stderr_tail[-8:]))
        client.close()

    if mode != "all":
        return

    heading("a second process with Null's MCP server attached to the session")
    client = AcpClient(omp, scratch)
    try:
        servers = [{
            "type": "http",
            "name": "null_spike",
            "url": "http://127.0.0.1:17493/mcp",
            "headers": [{"name": "X-Voicebox-Client-Id", "value": "null-mini-spike"}],
        }]
        _, _, new, new_s = open_session(client, servers)
        print(f"  session/new with mcpServers: {new_s} s → {short(new.get('result', {}).get('sessionId') or new.get('error'), 300)}")
        session_id = (new.get("result") or {}).get("sessionId")
        if session_id:
            show_prompt_result(client.prompt(
                session_id,
                "Use the list_profiles tool from the MCP server named null_spike, then reply with only the "
                "number of profiles it returned. If you have no such tool, reply with exactly: NO TOOL"))
    finally:
        client.close()


if __name__ == "__main__":
    main()
