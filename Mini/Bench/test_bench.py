"""The bench's own checks, run with no model and no Chrome:

    python3 -m unittest discover -s Mini/Bench

Above all, every checker is tried on a right and a wrong end state of its own
before any run is measured on it: a checker that passed a wrong end state, or
failed a right one, would make every number the bench gives wrong.
"""

import hashlib
import json
import os
import socket
import subprocess
import sys
import threading
import unittest
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bench  # noqa: E402
import grade  # noqa: E402
import isolation  # noqa: E402
import replay  # noqa: E402
import server  # noqa: E402
import session  # noqa: E402
import world  # noqa: E402

AXES = {
    "where": {"signed-in", "two-sites"},
    "job": {"find", "count", "form", "write", "change", "act", "workflow"},
    "length": {"short", "middle", "long"},
    "seeing": {"text", "drawn", "icons", "late"},
    "clarity": {"clear", "two-matches", "impossible"},
    "trouble": {"none", "popup", "slow", "expired", "fails"},
    "safety": {"none", "hidden-instruction", "ask-first"},
}


class EveryChecker(unittest.TestCase):
    def test_passes_its_right_end_state_and_fails_every_wrong_one(self):
        for task in bench.tasks():
            with self.subTest(task=task["id"]):
                end, told, reply = replay.end_state(task["right"])
                self.assertEqual([t for t in told if t[1] != 200], [], "the right end state is one the pages can reach")
                judged = grade.grade(task, end, reply)
                self.assertTrue(judged["ok"], [c for c in judged["checks"] if not c["ok"]])
                self.assertTrue(task["wrong"], "a task has at least one wrong end state")
                for wrong in task["wrong"]:
                    end, _, reply = replay.end_state(wrong)
                    self.assertFalse(grade.grade(task, end, reply)["ok"], f"a wrong end state passes: {wrong['reply']}")

    def test_an_untouched_world_with_no_answer_is_never_right(self):
        for task in bench.tasks():
            with self.subTest(task=task["id"]):
                end, _, _ = replay.end_state({"do": [], "reply": ""})
                self.assertFalse(grade.grade(task, end, "")["ok"], "doing nothing and saying nothing passes")

    def test_judges_a_run_that_removed_what_a_check_looks_at_as_wrong_and_goes_on(self):
        shared = next(t for t in bench.tasks() if t["id"] == "docs-share-trip")
        end, _, _ = replay.end_state({"do": [["docs", "delete", {"id": "d08"}]], "reply": "Shared."})
        judged = grade.grade(shared, end, "Shared.")
        self.assertFalse(judged["ok"])
        self.assertIn("not there any more", judged["checks"][0]["why"])


class EveryTask(unittest.TestCase):
    def test_is_whole_and_tagged_on_every_axis(self):
        for task in bench.tasks():
            with self.subTest(task=task["id"]):
                self.assertTrue(set(task["sites"]) <= set(server.SITES) and task["sites"])
                self.assertEqual(set(task["axes"]), set(AXES))
                for axis, value in task["axes"].items():
                    self.assertIn(value, AXES[axis], axis)
                self.assertEqual(task["tier"], "long" if task["axes"]["length"] == "long" else "everyday")
                for item in task["check"]:
                    (kind, _), = item.items()
                    self.assertIn(kind, grade.CHECKS)

    def test_names_no_real_address(self):
        text = json.dumps(bench.tasks()) + json.dumps(world.World().state)
        for address in __import__("re").findall(r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+", text):
            self.assertTrue(address.endswith(".test"), address)

    def test_the_request_says_where_the_person_s_sites_are(self):
        task = next(t for t in bench.tasks() if len(t["sites"]) == 2)
        said = bench.request(task, "http://localhost:8790")
        self.assertTrue(said.startswith(task["request"]))
        for site in task["sites"]:
            self.assertIn(f"http://localhost:8790/{site}", said)


class TheSplit(unittest.TestCase):
    def test_is_the_one_drawn_before_any_run(self):
        with open(os.path.join(bench.TASKS, "Split.json")) as f:
            record = json.load(f)
        every = {t["id"]: t for t in bench.tasks()}
        # The first draw, and each set of tasks added since, each drawn by its own seed before any run.
        draws = [(record["seed"], record["visible"]["ids"], record["hidden"]["ids"])] + [(added["seed"], added["visible"]["ids"], added["hidden"]["ids"]) for added in record.get("added", [])]
        all_visible, all_hidden = [], []
        for seed, visible, hidden in draws:
            drawn = [every[i] for i in visible + hidden]
            self.assertEqual(bench.split(drawn, seed), (sorted(visible), sorted(hidden)))
            all_visible += visible
            all_hidden += hidden
        self.assertEqual(sorted(t["id"] for t in bench.tasks("visible")), sorted(all_visible))
        self.assertEqual(sorted(t["id"] for t in bench.tasks("hidden")), sorted(all_hidden))
        with open(os.path.join(bench.TASKS, "Hidden.json"), "rb") as f:
            self.assertEqual(hashlib.sha256(f.read()).hexdigest(), record["hidden"]["sha256"], "the hidden half was changed after it was drawn")

    def test_holds_about_sixty_everyday_tasks_in_each_half(self):
        for half in ("visible", "hidden"):
            self.assertGreaterEqual(sum(t["tier"] == "everyday" for t in bench.tasks(half)), 60, half)


class TheRules(unittest.TestCase):
    def test_are_joined_as_null_joins_them(self):
        read = lambda name: open(os.path.join(bench.MINI, "Rules", name), encoding="utf-8").read()
        self.assertEqual(bench.rules("plain"), read("Computer.md"))
        self.assertEqual(bench.rules("link"), read("Computer.md") + "\n" + read("Browser.md"))


class TheSandbox(unittest.TestCase):
    def test_shuts_every_way_to_the_person_s_chrome_and_opens_only_the_bench_s_ports(self):
        text = isolation.profile(8790, 9334, hidden=["/x/Tasks"], readable=["/x/runs/this"])
        for rule in ["(deny appleevent-send)", "(deny lsopen)", '(deny network-outbound (remote ip "*:*"))', '(allow network-outbound (remote ip "*:443"))',
                     '(allow network-outbound (remote ip "localhost:8790"))', '(allow network-outbound (remote ip "localhost:9334"))', '(deny file-read-data (subpath "/x/Tasks"))',
                     '(deny process-exec (subpath "/Applications/Google Chrome.app"))', '(deny process-exec (literal "/usr/sbin/screencapture"))']:
            self.assertIn(rule, text)
        self.assertNotIn("9224", text.replace(";;", "\n;;").split("(deny network-outbound (remote ip")[1])
        allowed = [line for line in text.splitlines() if line.startswith("(allow")]
        self.assertEqual(len(allowed), 5, allowed)
        self.assertEqual(allowed[-1], '(allow file-read-data (subpath "/x/runs/this"))', "the run's own folder, and only after what is hidden")

    def test_never_opens_the_port_a_person_s_chrome_links_to(self):
        with self.assertRaises(AssertionError):
            isolation.profile(8790, isolation.PERSONS_RELAY)


class TheSessionFile(unittest.TestCase):
    def test_gives_the_run_s_numbers_and_its_last_words(self):
        entries = [
            {"type": "session", "id": "s"},
            {"type": "message", "message": {"role": "user", "content": [{"type": "text", "text": "hi"}], "timestamp": 1_791_000_000_000}},
            {"type": "message", "message": {"role": "assistant", "content": [{"type": "toolCall", "id": "1", "name": "eval", "arguments": {}}], "timestamp": 1_791_000_002_000, "usage": {"input": 10, "output": 5}}},
            {"type": "message", "message": {"role": "toolResult", "toolCallId": "1", "content": [{"type": "image", "data": "x"}], "isError": False, "timestamp": 1_791_000_005_000}},
            {"type": "message", "message": {"role": "assistant", "content": [{"type": "toolCall", "id": "2", "name": "bash", "arguments": {}}], "timestamp": 1_791_000_006_000}},
            {"type": "message", "message": {"role": "toolResult", "toolCallId": "2", "content": [{"type": "text", "text": "no"}], "isError": True, "timestamp": 1_791_000_007_000}},
            {"type": "message", "message": {"role": "assistant", "content": [{"type": "thinking", "thinking": "..."}, {"type": "text", "text": "Five."}], "timestamp": 1_791_000_010_500, "model": "m", "provider": "p", "stopReason": "stop"}},
        ]
        got = session.numbers(entries)
        self.assertEqual((got["seconds"], got["steps"], got["failed_steps"], got["turns"], got["pictures"], got["answer"], got["model"]), (10.5, 2, 1, 3, 1, "Five.", "p/m"))
        self.assertEqual(got["tools"], {"eval": 1, "bash": 1})


class TheWorld(unittest.TestCase):
    def test_lists_mail_a_page_at_a_time(self):
        w = world.World()
        first = w.view("mail", "list", {"folder": "inbox"})[1]
        self.assertEqual((len(first["rows"]), first["pages"]), (25, 3))
        self.assertEqual(w.view("mail", "list", {"q": "increase"})[1]["total"], 0, "a search is for the words a letter has")

    def test_keeps_only_plain_formatting(self):
        self.assertEqual(world.clean_html('<p onclick="x()">a<script>bad()</script><a href="x">b</a></p>'), "<p>ab</p>")
        self.assertEqual(world.clean_html('<p style="text-align: center; position: fixed">c</p>'), '<p style="text-align: center">c</p>')

    def test_reads_times_as_a_person_writes_them(self):
        self.assertEqual([world.hhmm(t) for t in ("5:30 pm", "17:30", "9", "12am")], ["17:30", "17:30", "09:00", "00:00"])

    def test_signs_out_and_fails_on_purpose_when_told_to(self):
        w = world.World(trouble={"expire_after": 1, "fail_once": ["chat.send"]})
        self.assertEqual(w.act("chat", "send", {"channel": "c-team", "text": "a"})[0], 500)
        self.assertEqual(w.act("chat", "send", {"channel": "c-team", "text": "a"})[0], 401)


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class TheServer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.port = free_port()
        cls.server, cls.bench = server.serve(cls.port, "runner-key")
        threading.Thread(target=cls.server.serve_forever, daemon=True).start()
        cls.base = f"http://127.0.0.1:{cls.port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()

    def status(self, path, method="GET", cookie=None, body=None):
        req = urllib.request.Request(self.base + path, method=method, data=json.dumps(body).encode() if body is not None else None, headers={"Cookie": cookie} if cookie else {})
        try:
            with urllib.request.urlopen(req) as answer:
                return answer.status, answer.read()
        except urllib.error.HTTPError as e:
            return e.code, e.read()

    def test_keeps_the_runner_s_controls_to_the_runner(self):
        self.assertEqual(self.status("/__bench/end")[0], 403)
        self.assertEqual(self.status("/__bench/reset", "POST", body={})[0], 403)
        self.assertEqual(self.status("/__bench/end?key=runner-key")[0], 200)

    def test_shows_a_browser_that_is_not_signed_in_the_sign_in_page(self):
        status, page = self.status("/mail")
        self.assertEqual(status, 200)
        self.assertIn(b"Sign in to continue", page)
        self.assertEqual(self.status("/api/mail/list")[0], 401)
        self.assertEqual(self.status("/api/mail/send", "POST", body={"to": "x@y.test"})[0], 401)
        signed_in = f"{server.COOKIE}={self.bench.session}"
        self.assertIn(b"Postbox", self.status("/mail", cookie=signed_in)[1])
        self.assertEqual(self.status("/api/mail/list", cookie=signed_in)[0], 200)

    def test_opens_only_web_addresses(self):
        self.assertEqual(self.status("/__bench/open?url=file:///etc/passwd")[0], 400)


class TheOpenStandIn(unittest.TestCase):
    def test_takes_only_a_web_address_for_chrome(self):
        shim = os.path.join(HERE, "Shim", "open")
        env = dict(os.environ, NULL_BENCH_PAGES="http://127.0.0.1:9")
        for argv in (["-a", "TextEdit", "/tmp/x.txt"], ["/tmp/x.txt"], ["-a", "Safari", "https://example.test"]):
            done = subprocess.run([shim, *argv], env=env, capture_output=True, text=True)
            self.assertNotEqual(done.returncode, 0, argv)
            self.assertIn("only a web address", done.stderr)


if __name__ == "__main__":
    unittest.main()
