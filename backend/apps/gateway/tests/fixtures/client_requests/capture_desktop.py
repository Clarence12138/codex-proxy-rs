"""固定 Desktop ZIP 的公开制品核验及隔离 app-server 捕获；需要 macOS / Python 3.14。"""
import argparse
import base64
from compression import zstd
from datetime import datetime, timezone
import hashlib
import http.server
import json
from pathlib import Path
import plistlib
import queue
import re
import struct
import subprocess
import tempfile
import threading
import zipfile

VERSION = "26.908.40834"
BUILD = "8881"
CORE_VERSION = "0.154.0-alpha.6.2"
CORE_SHA256 = "ecad78dbf98adb89ec475edac86630406cbe59d9f3070b17d88065f136b94bcb"
ASAR_SHA256 = "bb40cd8811887363104a19291346af9595632e0e956316a1086b274fb8e3eafc"
COMMIT = "b5bffd3ec4db487e7e3dec59663875b0ef7b72ca"


def events():
    return [{"type": "response.completed", "response": {
        "id": "resp_desktop_audit", "object": "response", "model": "gpt-5.5",
        "status": "completed", "output": [],
        "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2},
    }}]


def frame(opcode, payload):
    size = len(payload)
    head = bytes([128 | opcode, size]) if size < 126 else bytes([128 | opcode, 126]) + struct.pack("!H", size)
    return head + payload


def read_exact(stream, size):
    result = b""
    while len(result) < size:
        chunk = stream.read(size - len(result))
        if not chunk:
            raise EOFError("closed WebSocket")
        result += chunk
    return result


def capture(core, directory, transport):
    home = directory / transport
    work = home / "workspace"
    work.mkdir(parents=True)
    requests = []
    blocked = []

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *_):
            pass

        def reject(self, status):
            self.send_response(status)
            self.send_header("Content-Length", "0")
            self.end_headers()

        def do_CONNECT(self):
            # 外网回退只能到这个拒绝代理；永不转发 CONNECT 或绝对 URL。
            blocked.append(self.path)
            self.reject(502)

        def local_request(self):
            if self.path.startswith(("http:", "https:")):
                blocked.append(self.path)
                self.reject(502)
                return False
            return True

        def record(self, body):
            headers = []
            for name, value in self.headers.items():
                name = name.lower()
                if name == "host":
                    value = "127.0.0.1:1"
                elif name == "sec-websocket-key":
                    value = "c3ludGhldGljLWtleS0xNg=="
                headers.append([name, value])
            requests.append({"headers": headers, "body": body, "response_events": events()})

        def do_GET(self):
            if not self.local_request():
                return
            if self.headers.get("upgrade", "").lower() != "websocket":
                data = b'{"models":[]}'
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            if transport == "sse":
                # 使用 Core 自身的 HTTP fallback，不覆盖保留的内置 OpenAI provider。
                self.reject(426)
                return
            accept = base64.b64encode(hashlib.sha1((self.headers["sec-websocket-key"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            self.send_response(101)
            self.send_header("Upgrade", "websocket")
            self.send_header("Connection", "Upgrade")
            self.send_header("Sec-WebSocket-Accept", accept)
            self.end_headers()
            self.connection.settimeout(30)
            try:
                while True:
                    first, second = read_exact(self.rfile, 2)
                    assert first & 128 and not first & 64, "fixture server does not negotiate deflate"
                    opcode, size = first & 15, second & 127
                    if size == 126:
                        size = struct.unpack("!H", read_exact(self.rfile, 2))[0]
                    elif size == 127:
                        size = struct.unpack("!Q", read_exact(self.rfile, 8))[0]
                    assert second & 128 and size < 2 ** 24
                    mask = read_exact(self.rfile, 4)
                    payload = bytes(value ^ mask[i % 4] for i, value in enumerate(read_exact(self.rfile, size)))
                    if opcode == 8:
                        return
                    if opcode == 9:
                        self.wfile.write(frame(10, payload))
                        continue
                    assert opcode == 1
                    self.record(json.loads(payload))
                    for event in events():
                        self.wfile.write(frame(1, json.dumps(event).encode()))
                    self.wfile.flush()
            except (EOFError, ConnectionError, TimeoutError):
                return

        def do_POST(self):
            if not self.local_request():
                return
            data = self.rfile.read(int(self.headers["content-length"]))
            if self.path != "/backend-api/codex/responses":
                self.reject(404)
                return
            assert transport == "sse", "unexpected HTTP fallback"
            if self.headers.get("content-encoding") == "zstd":
                data = zstd.decompress(data)
            self.record(json.loads(data))
            data = "".join(f"event: {event['type']}\ndata: {json.dumps(event)}\n\n" for event in events()).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    threading.Thread(target=server.serve_forever, daemon=True).start()
    endpoint = f"http://127.0.0.1:{server.server_port}"
    # 两个 URL 都必须覆盖：chatgpt_base_url 不负责模型接口的地址。
    (home / "config.toml").write_text(f'openai_base_url = "{endpoint}/backend-api/codex"\nchatgpt_base_url = "{endpoint}/backend-api"\n[analytics]\nenabled=false\n[feedback]\nenabled=false\n')
    env = {"HOME": str(home), "CODEX_HOME": str(home), "TMPDIR": str(home), "PATH": "/usr/bin:/bin", "CODEX_INTERNAL_ORIGINATOR_OVERRIDE": "Codex Desktop", "HTTP_PROXY": endpoint, "HTTPS_PROXY": endpoint, "ALL_PROXY": endpoint, "NO_PROXY": "127.0.0.1,localhost"}
    sandbox = f'(version 1)(allow default)(deny network*)(allow network-outbound (remote ip "localhost:{server.server_port}"))(allow network-bind network-inbound (local ip "localhost:*"))'
    messages = queue.Queue()
    stderr = (home / "stderr.log").open("w")
    process = subprocess.Popen(["/usr/bin/sandbox-exec", "-p", sandbox, str(core), "app-server"], env=env, cwd=work, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, text=True)

    def reader():
        for line in process.stdout:
            messages.put(json.loads(line))
        messages.put(None)

    threading.Thread(target=reader, daemon=True).start()

    def rpc(number, method, params):
        process.stdin.write(json.dumps({"id": number, "method": method, "params": params}) + "\n")
        process.stdin.flush()
        while True:
            message = messages.get(timeout=45)
            if message is None:
                raise RuntimeError((home / "stderr.log").read_text()[:2000])
            if message.get("id") == number:
                assert "error" not in message, message.get("error")
                return message["result"]

    try:
        initialized = rpc(1, "initialize", {"clientInfo": {"name": "Codex Desktop", "title": "Codex Desktop", "version": VERSION}, "capabilities": {"experimentalApi": True, "requestAttestation": False}})
        process.stdin.write('{"method":"initialized","params":{}}\n')
        process.stdin.flush()
        claims = {"sub": "synthetic-user", "exp": 4102444800, "https://api.openai.com/auth": {"chatgpt_account_id": "synthetic-account", "chatgpt_plan_type": "plus", "chatgpt_user_id": "synthetic-user"}}
        token = "eyJhbGciOiJub25lIn0." + base64.urlsafe_b64encode(json.dumps(claims).encode()).decode().rstrip("=") + ".synthetic"
        rpc(2, "account/login/start", {"type": "chatgptAuthTokens", "accessToken": token, "chatgptAccountId": "synthetic-account", "chatgptPlanType": "plus"})
        thread = rpc(3, "thread/start", {"model": "gpt-5.5", "cwd": str(work), "approvalPolicy": "never", "sandbox": "read-only", "baseInstructions": "Synthetic coding assistant.", "developerInstructions": "Synthetic audit; use no tools."})
        rpc(4, "turn/start", {"threadId": thread["thread"]["id"], "input": [{"type": "text", "text": "中文合成测试，请回答完成。", "text_elements": []}]})
        while True:
            message = messages.get(timeout=45)
            if message.get("method") == "turn/completed":
                assert message["params"]["turn"]["status"] == "completed", message
                break
        assert requests, f"no local model request; rejected external destinations: {blocked}"
        if blocked:
            print(f"Rejected (never forwarded) auxiliary network requests: {blocked}")
        for request in requests:
            assert dict(request["headers"])["version"] == CORE_VERSION
            assert dict(request["headers"])["user-agent"] == initialized["userAgent"]
        serialized = json.dumps(requests, ensure_ascii=False).replace(str(work.resolve()), "/synthetic/workspace").replace(str(work), "/synthetic/workspace").replace(str(home.resolve()), "/synthetic/codex-home").replace(str(home), "/synthetic/codex-home")
        return initialized["userAgent"], {"name": f"desktop-bundled-core-{transport}", "api": "openai-codex-responses", "transport": transport, "prompt_kind": "desktop", "rejected_auxiliary_destinations": sorted(set(blocked)), "turns": json.loads(serialized)}
    finally:
        process.terminate()
        process.wait(timeout=10)
        stderr.close()
        server.shutdown()
        server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("zip", type=Path, help="explicit local official Desktop ZIP; no download or install")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    with zipfile.ZipFile(args.zip) as archive, tempfile.TemporaryDirectory(prefix="codex-desktop-capture-") as temporary:
        root = Path(temporary)
        info = plistlib.loads(archive.read("ChatGPT.app/Contents/Info.plist"))
        assert info["CFBundleShortVersionString"] == VERSION and info["CFBundleVersion"] == BUILD
        core_data = archive.read("ChatGPT.app/Contents/Resources/codex")
        asar = archive.read("ChatGPT.app/Contents/Resources/app.asar")
        assert hashlib.sha256(core_data).hexdigest() == CORE_SHA256
        assert hashlib.sha256(asar).hexdigest() == ASAR_SHA256
        # 不执行 Electron 代码；这些稳定片段关联 Desktop 身份、初始化和子进程环境。
        for needle in [b'clientInfo:{name:r.tt,title:`Codex Desktop`,version:p}', b'CODEX_INTERNAL_ORIGINATOR_OVERRIDE:e.defaultOriginator??rU', b'rU=`Codex Desktop`', b'"tt",{enumerable:!0,get:function(){return lw}}', b'lw=`Codex Desktop`']:
            assert needle in asar, needle
        core = root / "codex"
        core.write_bytes(core_data)
        core.chmod(0o700)
        version = subprocess.check_output([str(core), "--version"], env={"HOME": temporary, "CODEX_HOME": temporary, "PATH": "/usr/bin:/bin"}, text=True).strip()
        assert version == f"codex-cli {CORE_VERSION}"
        samples = []
        for transport in ["sse", "websocket"]:
            user_agent, sample = capture(core, root, transport)
            samples.append(sample)
        match = re.fullmatch(r'Codex Desktop/(.+) \(Mac OS (.+); (.+)\) (.+) \(Codex Desktop; (.+)\)', user_agent)
        assert match
        fixture = {"source": {
            "desktop_version": VERSION, "desktop_build": BUILD, "core_version": CORE_VERSION,
            "core_sha256": CORE_SHA256, "asar_sha256": ASAR_SHA256, "core_source_commit": COMMIT,
            "artifact_url": f"https://persistent.oaistatic.com/codex-app-prod/ChatGPT-darwin-arm64-{VERSION}.zip",
            "generator": "capture_desktop.py", "evidence": "bundled app-server, not full GUI App",
            "isolation": "fresh HOME/CODEX_HOME, explicit openai_base_url and chatgpt_base_url, loopback-only sandbox and rejecting proxy",
            "test_overrides": "synthetic external ChatGPT tokens, analytics/feedback disabled, requestAttestation false, synthetic instructions, empty local model catalog; SSE via intentional WS 426",
            "normalization": "decompressed HTTP body; generated workspace/home paths, host port and WebSocket key normalized; generated UUIDs retained",
        }, "profile": {"originator": "Codex Desktop", "codex_version": match[1], "desktop_version": VERSION, "desktop_build": BUILD, "os_type": "Mac OS", "os_version": match[2], "arch": match[3], "terminal": match[4], "residency": None, "location": None, "verified_at": datetime.now(timezone.utc).isoformat()}, "samples": samples}
        args.output.write_text(json.dumps(fixture, ensure_ascii=False, indent=2) + "\n")
        print(f"Captured Desktop {VERSION} / bundled Core {CORE_VERSION}: HTTP and WS")


if __name__ == "__main__":
    main()
