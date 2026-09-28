#!/usr/bin/env python3
"""End-to-end run of the installed OpenCompanion desktop app on Linux (e2e/linux/inside.sh).

Drives the app through tauri-driver (W3C WebDriver -> WebKitWebDriver -> WebKitGTK), with
stand-in `claude` and `opencode` executables that exec fake-cli and are on the login shell's
PATH only. Each step records PASS/FAIL and a screenshot, and the run continues, so one broken
screen does not hide the rest. Plain standard library: no pip install needed.
"""
import base64
import json
import os
import re
import subprocess
import sys
import time
import traceback
import urllib.error
import urllib.request

DRIVER = "http://127.0.0.1:4444"
APP = os.environ.get("OC_APP", "/usr/bin/opencompanion")
OUT = os.environ.get("OC_OUT", "/out")
FAKES = "/root/.local/share/oc-fakes/bin"
CONTROL, SHIFT = "", ""
PROJECT = os.environ.get("OC_PROJECT", "/root/e2e-project")
ELEMENT = "element-6066-11e4-a52e-4f735466cecf"
os.makedirs(OUT, exist_ok=True)

results = []
shot_no = [0]


def http(method, path, body=None, timeout=60):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(DRIVER + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read() or b"{}").get("value")
    except urllib.error.HTTPError as e:
        payload = e.read().decode(errors="replace")
        raise RuntimeError(f"{method} {path} -> {e.code}: {payload[:400]}")


class App:
    def __init__(self):
        caps = {"capabilities": {"alwaysMatch": {"browserName": "wry",
                                                 "tauri:options": {"application": APP}}}}
        value = http("POST", "/session", caps, timeout=120)
        self.sid = value["sessionId"]

    def p(self, suffix):
        return f"/session/{self.sid}{suffix}"

    def quit(self):
        try:
            http("DELETE", self.p(""))
        except Exception as e:  # the app may already be gone
            print("quit:", e)

    def js(self, script, *args):
        return http("POST", self.p("/execute/sync"), {"script": script, "args": list(args)})

    def find(self, css):
        v = http("POST", self.p("/element"), {"using": "css selector", "value": css})
        return v[ELEMENT]

    def find_all(self, css):
        v = http("POST", self.p("/elements"), {"using": "css selector", "value": css})
        return [e[ELEMENT] for e in v]

    def xfind(self, xpath):
        v = http("POST", self.p("/element"), {"using": "xpath", "value": xpath})
        return v[ELEMENT]

    def click(self, el):
        http("POST", self.p(f"/element/{el}/click"), {})

    def type(self, el, text):
        http("POST", self.p(f"/element/{el}/value"), {"text": text})

    def clear(self, el):
        http("POST", self.p(f"/element/{el}/clear"), {})

    def text_of(self, el):
        return http("GET", self.p(f"/element/{el}/text"))

    def body(self):
        return self.js("return document.body ? document.body.innerText : ''") or ""

    def title(self):
        return http("GET", self.p("/title"))

    def url(self):
        return self.js("return location.href")

    def shot(self, name):
        shot_no[0] += 1
        path = os.path.join(OUT, f"{shot_no[0]:02d}-{name}.png")
        try:
            png = http("GET", self.p("/screenshot"))
            with open(path, "wb") as f:
                f.write(base64.b64decode(png))
        except Exception as e:
            print("screenshot failed:", e)
        return path

    def wait(self, pred, timeout=20, what="condition"):
        end = time.time() + timeout
        last = None
        while time.time() < end:
            try:
                v = pred()
                if v:
                    return v
            except Exception as e:
                last = e
            time.sleep(0.3)
        raise AssertionError(f"timed out waiting for {what}" + (f" (last error: {last})" if last else ""))

    def wait_text(self, text, timeout=20):
        return self.wait(lambda: text in self.body(), timeout, f"text {text!r}")

    def wait_css(self, css, timeout=20):
        return self.wait(lambda: self.find(css), timeout, f"element {css}")

    def click_css(self, css, timeout=20):
        el = self.wait_css(css, timeout)
        self.click(el)
        return el

    def click_button(self, label, timeout=20):
        xp = f"//button[not(@disabled)][contains(normalize-space(.), {xq(label)})]"
        el = self.wait(lambda: self.xfind(xp), timeout, f"button {label!r}")
        self.click(el)
        return el

    def chord(self, *keys):
        """Presses the keys together, as a person holds Ctrl+Shift and taps V."""
        down = [{"type": "keyDown", "value": k} for k in keys]
        up = [{"type": "keyUp", "value": k} for k in reversed(keys)]
        http("POST", self.p("/actions"), {"actions": [{"type": "key", "id": "keyboard", "actions": down + up}]})
        http("DELETE", self.p("/actions"))

    def jsclick(self, css):
        ok = self.js("const e=document.querySelector(arguments[0]); if(!e) return false; e.click(); return true;", css)
        if not ok:
            raise AssertionError(f"no element {css}")

    def nav(self, href):
        """Client-side navigation through a real link, as a click in the sidebar does."""
        self.click_css(f'a[href="{href}"]')
        self.wait(lambda: self.js("return location.pathname") == href.split("?")[0], 10, f"route {href}")


# The shell tab shown on a session's screen.
SHELL_TAB = "section[id^=shell-panel-]:not([hidden])"


def xq(s):
    return "'" + s + "'" if "'" not in s else 'concat("' + s.replace('"', '') + '")'


def step(name, app=None):
    def deco(fn):
        t0 = time.time()
        try:
            detail = fn() or ""
            status = "PASS"
        except Exception as e:
            status = "FAIL"
            detail = f"{type(e).__name__}: {e}"
            traceback.print_exc()
        shot = app.shot(re.sub(r"[^a-z0-9]+", "-", name.lower())) if app else ""
        results.append({"step": name, "status": status, "detail": str(detail)[:600],
                        "seconds": round(time.time() - t0, 1), "shot": os.path.basename(shot)})
        print(f"[{status}] {name} ({results[-1]['seconds']}s) {detail}", flush=True)
        return fn
    return deco


def main():
    os.makedirs(PROJECT, exist_ok=True)

    @step("setup: the fake CLIs are on the login shell's PATH only", None)
    def _():
        assert FAKES not in os.environ.get("PATH", ""), "the app would inherit the fakes' folder"
        login = subprocess.run(["bash", "-ilc", "command -v claude"], capture_output=True, text=True, timeout=20)
        assert login.stdout.strip() == f"{FAKES}/claude", login.stdout + login.stderr
        return "app PATH lacks them; bash -ilc finds them"

    app = App()
    print("session", app.sid, flush=True)

    @step("launch: window loads the SvelteKit shell", app)
    def _():
        app.wait(lambda: app.js("return document.readyState") == "complete", 30, "document ready")
        app.wait(lambda: app.find_all("#onboarding, #overview-main, #titlebar"), 30, "app shell")
        return f"url={app.url()} title={app.title()!r}"

    @step("onboarding: fake Claude Code and OpenCode are detected", app)
    def _():
        app.wait_css("#onboarding", 30)
        app.wait(lambda: "2.1.99" in app.body() and "1.18.0" in app.body(), 40, "CLI versions listed")
        body = app.body()
        assert "Claude Code" in body and "OpenCode" in body, body[:500]
        return "versions 2.1.99 (claude) and 1.18.0 (opencode) shown"

    @step("onboarding: continue lands on Overview", app)
    def _():
        app.click_css("#ob-continue:not([disabled])")
        app.wait(lambda: app.js("return location.pathname") == "/", 15, "overview route")
        app.wait_css("#overview-main")
        return app.title()

    @step("new session: headless OpenCode turn finishes with the fake output", app)
    def _():
        app.click_css("#btn-new-session")
        app.wait_css("#ns-folder")
        app.jsclick('input[name="ns-cli"][value="opencode"]')
        app.type(app.find("#ns-folder"), PROJECT)
        app.jsclick('input[name="ns-mode"][value="headless"]')
        app.type(app.wait_css("#ns-prompt"), "hello from linux e2e")
        app.click_button("Start OpenCode in e2e-project")
        app.wait(lambda: app.js("return location.pathname") == "/session", 15, "session route")
        app.wait_text("done: hello from linux e2e", 30)
        app.wait(lambda: re.search(r"\b(done|idle)\b", app.body()), 20, "finished status")
        return "timeline shows 'done: hello from linux e2e'"

    @step("new session: headless Claude asks permission, Approve lets it continue", app)
    def _():
        app.jsclick('button.side-add')
        app.wait_css("#ns-folder")
        app.jsclick('input[name="ns-cli"][value="claude"]')
        app.type(app.find("#ns-folder"), PROJECT)
        app.jsclick('input[name="ns-mode"][value="headless"]')
        app.type(app.wait_css("#ns-prompt"), "write hello.txt")
        app.js("const s=document.querySelector('#ns-perm'); s.value='ask'; s.dispatchEvent(new Event('change',{bubbles:true}));")
        app.click_button("Start Claude Code in e2e-project")
        app.wait(lambda: app.js("return location.pathname") == "/session", 15, "session route")
        app.click_button("Approve", 30)
        app.wait_text("allowed", 30)
        return "permission request answered, CLI replied 'allowed'"

    @step("new session: interactive Claude runs in a PTY and echoes typed input", app)
    def _():
        app.jsclick('button.side-add')
        app.wait_css("#ns-folder")
        app.jsclick('input[name="ns-cli"][value="claude"]')
        app.type(app.find("#ns-folder"), PROJECT)
        app.jsclick('input[name="ns-mode"][value="interactive"]')
        app.click_button("Start Claude Code in e2e-project")
        app.wait(lambda: app.js("return location.pathname") == "/session", 15, "session route")
        rows = lambda: app.js("return [...document.querySelectorAll('.xterm-rows')].map(r=>r.innerText).join('\\n')") or ""
        app.wait(lambda: "fake ready" in rows(), 30, "PTY prompt in xterm")
        ta = app.wait_css(".xterm-helper-textarea")
        app.type(ta, "ping-e2e\n")
        app.wait(lambda: "got: ping-e2e" in rows(), 20, "echo in xterm")
        return "xterm shows 'fake ready' then 'got: ping-e2e'"

    @step("shell tab: bash in the session's folder runs a command", app)
    def _():
        # Still on the interactive session: New terminal opens a shell tab in its folder.
        app.click_css("#btn-new-terminal")
        app.wait_css("#nt-shell")
        shells = app.js("return [...document.querySelectorAll('#nt-shell option')].map(o=>o.value+'='+o.textContent)")
        app.click_css("#btn-open-terminal:not([disabled])")
        rows = lambda: app.js(f"return [...document.querySelectorAll('{SHELL_TAB} .xterm-rows')].map(r=>r.innerText).join('\\n')") or ""
        ta = app.wait_css(f"{SHELL_TAB} .xterm-helper-textarea", 20)
        time.sleep(1.5)
        app.type(ta, "echo E2E_$((40+2)); pwd\n")
        app.wait(lambda: "E2E_42" in rows(), 20, "command output")
        assert PROJECT in rows(), "shell did not start in the session's folder"
        return f"shells={shells}; output has E2E_42 and cwd {PROJECT}"

    @step("shell tab: Ctrl+Shift+V pastes on Linux", app)
    def _():
        rows = lambda: app.js(f"return [...document.querySelectorAll('{SHELL_TAB} .xterm-rows')].map(r=>r.innerText).join('\\n')") or ""
        # xclip stays in the background to serve the clipboard; it must not hold our output open.
        subprocess.run(["xclip", "-selection", "clipboard"], input=b"echo PASTED_$((2+3))", check=True, timeout=10,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        app.js(f"document.querySelector('{SHELL_TAB} .xterm-helper-textarea').focus()")
        app.chord(CONTROL, SHIFT, "v")
        app.type(app.find(f"{SHELL_TAB} .xterm-helper-textarea"), "\n")
        app.wait(lambda: "PASTED_5" in rows(), 15, "pasted command ran")
        return "clipboard text pasted with Ctrl+Shift+V and ran: PASTED_5"

    @step("history: All sessions lists the three runs and search filters them", app)
    def _():
        app.nav("/")
        app.nav("/history")
        main = lambda: app.js("return document.querySelector('#history-main')?.innerText || ''") or ""
        app.wait(lambda: all(t in main() for t in ("hello from linux e2e", "write hello.txt", "ping-e2e")), 15, "three sessions listed")
        app.type(app.find("#history-main input[type=search]"), "ping")
        app.wait(lambda: "ping-e2e" in main() and "hello from linux e2e" not in main(), 10, "search keeps only ping-e2e")
        return "all three sessions listed; search 'ping' leaves only ping-e2e"

    @step("settings: switch language to Indonesian and back", app)
    def _():
        app.nav("/settings")
        app.wait_css("#lang-select")
        app.js("const s=document.querySelector('#lang-select'); s.value='id'; s.dispatchEvent(new Event('change',{bubbles:true}));")
        app.wait_text("Pengaturan", 10)
        app.js("const s=document.querySelector('#lang-select'); s.value='en'; s.dispatchEvent(new Event('change',{bubbles:true}));")
        app.wait_text("Settings", 10)
        return "UI switched to 'Pengaturan' and back to 'Settings'"

    phone = {}

    @step("settings: turn on phone access and get a pairing code", app)
    def _():
        app.wait_css("#phone")
        app.click_button("Turn on phone access")
        code_el = app.wait_css("#phone .code", 20)
        code = re.sub(r"\s+", "", app.text_of(code_el))
        assert re.fullmatch(r"\d{6}", code), code
        phone["code"] = code
        addr = app.js("const c=document.querySelector('#phone .addr code'); return c ? c.textContent : ''")
        phone["addr"] = addr
        return f"code={code} address={addr}"

    @step("phone companion: pair over HTTP, list sessions, serve /m", None)
    def _():
        port = 8765
        base = f"http://127.0.0.1:{port}"
        hello = json.loads(urllib.request.urlopen(base + "/api/hello", timeout=10).read())
        req = urllib.request.Request(base + "/api/pair", method="POST",
                                     data=json.dumps({"code": phone["code"], "name": "e2e phone"}).encode(),
                                     headers={"Content-Type": "application/json"})
        paired = json.loads(urllib.request.urlopen(req, timeout=10).read())
        token = paired.get("token")
        assert token, paired
        req = urllib.request.Request(base + "/api/sessions", headers={"Authorization": f"Bearer {token}"})
        sessions = json.loads(urllib.request.urlopen(req, timeout=10).read())
        items = sessions if isinstance(sessions, list) else sessions.get("sessions", sessions)
        page = urllib.request.urlopen(base + "/m", timeout=10)
        html = page.read().decode(errors="replace")
        assert "<html" in html.lower(), html[:200]
        unauth = None
        try:
            urllib.request.urlopen(base + "/api/sessions", timeout=10)
        except urllib.error.HTTPError as e:
            unauth = e.code
        assert unauth == 401, f"unauthenticated /api/sessions gave {unauth}"
        return f"hello={json.dumps(hello)[:120]} sessions={len(items)} /m={page.status} unauth={unauth}"

    @step("settings: paired phone appears under devices", app)
    def _():
        app.wait_text("e2e phone", 15)
        return "device 'e2e phone' listed"

    @step("theme: Dusk theme applies a dark data-theme", app)
    def _():
        app.jsclick('#theme input[value="dark"]')
        app.wait(lambda: app.js("return document.documentElement.dataset.theme") == "dark", 10, "data-theme=dark")
        bg = app.js("return getComputedStyle(document.body).backgroundColor")
        app.shot("dusk-theme")
        app.jsclick('#theme input[value="light"]')
        app.wait(lambda: app.js("return document.documentElement.dataset.theme") == "light", 10, "data-theme=light")
        return f"dark applied (body bg {bg}), then back to light"

    @step("settings: start at login writes and removes the autostart entry", app)
    def _():
        entry = "/root/.config/autostart/opencompanion.desktop"
        box = "//label[contains(normalize-space(.), 'Start in the tray when I sign in')]//input[@type='checkbox']"
        app.js("arguments[0].scrollIntoView({block: 'center'})", {ELEMENT: app.xfind(box)})
        app.click(app.xfind(box))
        app.wait(lambda: os.path.isfile(entry), 10, "autostart entry written")
        text = open(entry, encoding="utf-8").read()
        assert 'Exec="/usr/bin/opencompanion" --hidden' in text, text
        app.click(app.xfind(box))
        app.wait(lambda: not os.path.exists(entry), 10, "autostart entry removed")
        return f"{entry} held Exec=\"/usr/bin/opencompanion\" --hidden, then was removed"

    @step("settings/CLIs: table shows the detected executables and versions", app)
    def _():
        app.nav("/settings/clis")
        app.wait_text(f"{FAKES}/claude", 20)
        app.wait_text(f"{FAKES}/opencode", 5)
        body = app.body()
        return f"paths listed; 'Not found' rows={body.count('Not found') + body.count('not found')}"

    @step("chat: page renders with Claude Code as planner", app)
    def _():
        app.nav("/chat")
        app.wait_css("#chat-main")
        app.wait_css("#composer-input")
        return "chat composer ready"

    def dunst(*args):
        return subprocess.run(["dunstctl", *args], capture_output=True, text=True, timeout=10).stdout.strip()

    @step("notifications: clicking a Linux notification opens its session", app)
    def _():
        dunst("close-all")
        app.nav("/")
        app.click_css("#btn-new-session")
        app.wait_css("#ns-folder")
        app.jsclick('input[name="ns-cli"][value="opencode"]')
        app.type(app.find("#ns-folder"), PROJECT)
        app.jsclick('input[name="ns-mode"][value="headless"]')
        app.type(app.wait_css("#ns-prompt"), "notify me")
        app.click_button("Start OpenCode in e2e-project")
        app.wait_text("done: notify me", 30)
        target = app.js("return location.search")
        app.nav("/history")
        app.wait(lambda: dunst("count", "displayed") not in ("", "0"), 15, "notification on screen")
        dunst("action", "0")
        app.wait(lambda: app.js("return location.pathname + location.search") == "/session" + target, 15, "session opened from the notification")
        return f"dunst showed the notification; its default action opened /session{target}"

    @step("window: titlebar buttons exist (custom decorations)", app)
    def _():
        ids = [i for i in ("btn-window-minimize", "btn-window-maximize", "btn-window-close") if app.find_all("#" + i)]
        assert len(ids) == 3, ids
        return ",".join(ids)

    app.quit()

    # Second launch against the same data folder: finished sessions and the paired device must
    # survive a restart.
    app2 = App()

    @step("restart: data survives (onboarding skipped, sessions, device)", app2)
    def _():
        app2.wait(lambda: app2.find_all("#overview-main, #onboarding"), 30, "shell")
        assert not app2.find_all("#onboarding"), "onboarding shown again after restart"
        app2.nav("/history")
        app2.wait_text("e2e-project", 15)
        app2.nav("/settings")
        app2.wait_text("e2e phone", 15)
        return "history and paired device both present"

    app2.quit()

    # Last, because it removes a library: a desktop without AppIndicator support must still get a
    # working app, and closing its window must quit instead of hiding it with no tray to return by.
    @step("tray: without an AppIndicator library the app starts, and closing it quits", None)
    def _():
        subprocess.run(["dpkg", "-r", "--force-depends", "libayatana-appindicator3-1"], check=True, capture_output=True, timeout=60)
        app3 = App()
        try:
            app3.wait(lambda: app3.find_all("#overview-main"), 30, "app shell")
            running = lambda: subprocess.run(["pgrep", "-x", "opencompanion"], capture_output=True).returncode == 0
            assert running(), "the app is not running"
            try:
                app3.click_css("#btn-window-close")
            except RuntimeError as e:
                # The app quits while the click is still being answered.
                assert "terminated" in str(e), e
            app3.wait(lambda: not running(), 15, "the app quit after its window closed")
        finally:
            app3.quit()
        return "started without the library; closing the window quit the app"


if __name__ == "__main__":
    try:
        main()
    except Exception:
        traceback.print_exc()
        results.append({"step": "harness", "status": "FAIL", "detail": traceback.format_exc()[-600:], "seconds": 0, "shot": ""})
    with open(os.path.join(OUT, "results.json"), "w") as f:
        json.dump(results, f, indent=2)
    passed = sum(r["status"] == "PASS" for r in results)
    print(f"\n{passed}/{len(results)} steps passed")
    sys.exit(0 if passed == len(results) else 1)
