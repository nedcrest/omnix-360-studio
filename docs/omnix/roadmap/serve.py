#!/usr/bin/env python3
"""Дорожня карта Omnix у браузері, що оновлюється сама.

Сторінка читає `docs/omnix/ROADMAP.md` і малює його картками. Сервер
стежить за файлом і, щойно той змінився, штовхає сторінці подію (SSE —
server-sent events): вона перечитує файл і перемальовується без
перезавантаження. Ще він віддає останні коміти Omnix і footage-agent.

Тільки стандартна бібліотека Python: нічого ставити не треба.

    python3 docs/omnix/roadmap/serve.py              → http://127.0.0.1:8770/
    python3 docs/omnix/roadmap/serve.py --install    (ярлик на робочий стіл)
"""

from __future__ import annotations

import argparse
import json
import socket
import subprocess
import sys
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent.parent
ROADMAP = REPO / "docs" / "omnix" / "ROADMAP.md"
PAGE = HERE / "index.html"
PORT = 8770
# Чиї коміти показувати: сам Omnix і конвеєр, що готує йому матеріал.
REPOS = {"Omnix": REPO, "footage-agent": Path.home() / "footage-agent"}
# Як часто дивитись на файл. Пів секунди — непомітно для ока й нічого не
# коштує: це один stat().
POLL_S = 0.5


def stamp() -> float:
    """Коли файл змінювався востаннє; 0, якщо його немає."""
    try:
        return ROADMAP.stat().st_mtime
    except OSError:
        return 0.0


def commits(limit: int = 8) -> dict[str, list[dict]]:
    """Останні коміти кожного репозиторію: хеш, коли, що."""
    out: dict[str, list[dict]] = {}
    for name, repo in REPOS.items():
        if not (repo / ".git").exists():
            continue
        r = subprocess.run(
            ["git", "-C", str(repo), "log", f"-{limit}", "--format=%h%x09%cs%x09%s"],
            capture_output=True, text=True, timeout=10, check=False)
        out[name] = [dict(zip(("hash", "date", "subject"), line.split("\t", 2)))
                     for line in r.stdout.splitlines() if line.count("\t") >= 2]
    return out


class Handler(BaseHTTPRequestHandler):
    server_version = "roadmap"

    def _send(self, body: bytes, ctype: str, code: int = 200) -> None:
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 — так його кличе http.server
        path = self.path.split("?", 1)[0]
        if path in ("/", "/index.html"):
            self._send(PAGE.read_bytes(), "text/html; charset=utf-8")
        elif path == "/roadmap.md":
            text = ROADMAP.read_text(encoding="utf-8") if ROADMAP.is_file() else ""
            self._send(text.encode(), "text/markdown; charset=utf-8")
        elif path == "/commits":
            self._send(json.dumps(commits(), ensure_ascii=False).encode(),
                       "application/json; charset=utf-8")
        elif path == "/events":
            self._events()
        else:
            self._send(b"not found", "text/plain", 404)

    def _events(self) -> None:
        """Потік подій: «changed», коли файл змінився; порожній рядок-пульс
        раз на 15 с, щоб з'єднання не вважалось мертвим."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        seen, beat = stamp(), time.monotonic()
        try:
            while True:
                time.sleep(POLL_S)
                now = stamp()
                if now != seen:
                    seen = now
                    self.wfile.write(b"data: changed\n\n")
                    self.wfile.flush()
                elif time.monotonic() - beat > 15:
                    beat = time.monotonic()
                    self.wfile.write(b": beat\n\n")
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            return   # вкладку закрили — це норма

    def log_message(self, fmt: str, *args: object) -> None:
        return   # тиша замість рядка на кожен запит


# ─────────────────────── ярлик на робочому столі ───────────────────────

ICON = "omnix-roadmap"
# Три колонки з картками: «зараз», «далі», «потім».
ICON_SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">
  <rect width="48" height="48" rx="10" fill="#2b2a2f"/>
  <g fill="#d29a5e"><rect x="7" y="9" width="10" height="8" rx="2"/>
    <rect x="7" y="20" width="10" height="8" rx="2"/></g>
  <g fill="#9a958d"><rect x="19" y="9" width="10" height="8" rx="2"/>
    <rect x="31" y="9" width="10" height="8" rx="2"/>
    <rect x="31" y="20" width="10" height="8" rx="2"/></g>
  <rect x="7" y="34" width="34" height="4" rx="2" fill="#7bbf8c"/>
</svg>
"""

ENTRY = """[Desktop Entry]
Type=Application
Name=Omnix — дорожня карта
Comment=Що робимо, що далі, що чекає перевірки
Exec={py} {script}
Icon={icon}
Terminal=false
Categories=Development;
"""


def install() -> list[Path]:
    """Ярлик у меню програм і на робочий стіл, з власною іконкою.

    ⚠️ GNOME показує ярлик на столі, лише позначений «довіреним»; KDE —
    лише виконуваний. Робимо обидва: chmod і `gio set metadata::trusted`
    (системний `/usr/bin/gio` — у Homebrew свій, без gvfs; так ламалось у
    студії footage-agent).
    """
    icons = Path.home() / ".local/share/icons/hicolor/scalable/apps"
    icons.mkdir(parents=True, exist_ok=True)
    (icons / f"{ICON}.svg").write_text(ICON_SVG, encoding="utf-8")
    body = ENTRY.format(py=sys.executable, script=Path(__file__).resolve(), icon=ICON)
    r = subprocess.run(["xdg-user-dir", "DESKTOP"], capture_output=True, text=True, check=False)
    desk = Path(r.stdout.strip()) if r.returncode == 0 and r.stdout.strip() else Path.home() / "Desktop"
    out = []
    for d in (Path.home() / ".local/share/applications", desk):
        d.mkdir(parents=True, exist_ok=True)
        f = d / "omnix-roadmap.desktop"
        f.write_text(body, encoding="utf-8")
        f.chmod(0o755)
        gio = "/usr/bin/gio" if Path("/usr/bin/gio").exists() else "gio"
        subprocess.run([gio, "set", "-t", "string", str(f), "metadata::trusted", "true"],
                       capture_output=True, check=False)
        out.append(f)
    return out


def _open(port: int) -> bool:
    with socket.socket() as s:
        s.settimeout(0.3)
        return s.connect_ex(("127.0.0.1", port)) == 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", type=int, default=PORT)
    ap.add_argument("--no-browser", action="store_true")
    ap.add_argument("--install", action="store_true", help="ярлик на робочий стіл і в меню")
    args = ap.parse_args()

    if args.install:
        for f in install():
            print(f"✓ ярлик: {f}")
        return 0

    url = f"http://127.0.0.1:{args.port}/"
    if _open(args.port):
        # Друге натискання на ярлик: сервер уже є, просто показати сторінку.
        if not args.no_browser:
            webbrowser.open(url)
        return 0
    srv = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    srv.daemon_threads = True   # відкриті вкладки не тримають сервер при виході
    print(f"Дорожня карта: {url}\nCtrl+C — вихід")
    if not args.no_browser:
        webbrowser.open(url)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        srv.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
