// 仅导入显式指定的程序包；不启动 Pi CLI，不读取用户配置或凭据。
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { zstdDecompressSync } from 'node:zlib';

const [aiPath, agentPath, outputPath] = process.argv.slice(2);
assert(aiPath && agentPath && outputPath, 'Usage: node capture_pi.mjs <pi-ai-dir> <pi-coding-agent-dir> <output.json>');
const ai = resolve(aiPath);
const agent = resolve(agentPath);
const requireAgent = createRequire(`${agent}/package.json`);
const aiPackage = JSON.parse(await readFile(`${ai}/package.json`, 'utf8'));
const agentPackage = JSON.parse(await readFile(`${agent}/package.json`, 'utf8'));
assert.equal(aiPackage.version, '0.85.1');
assert.equal(agentPackage.version, '0.85.1');
const undici = requireAgent('undici');
const { buildSystemPrompt } = await import(pathToFileURL(`${agent}/dist/core/system-prompt.js`));
process.env.PI_PACKAGE_DIR = '/synthetic/pi';
const responses = await import(pathToFileURL(`${ai}/dist/api/openai-responses.js`));
const codex = await import(pathToFileURL(`${ai}/dist/api/openai-codex-responses.js`));
// 禁止任何意外的外网 HTTP 或 WS fallback；仅显式 mock fetch 与下方 loopback WS 可用。
globalThis.fetch = async () => { throw new Error('Network fetch is forbidden'); };
const dispatcher = new undici.Agent();
const sockets = new Set();
let allowedWsUrl;
globalThis.WebSocket = class extends undici.WebSocket {
  constructor(url, options) {
    assert.equal(String(url), allowedWsUrl, 'Only the capture server may be contacted');
    super(url, { ...options, dispatcher });
    sockets.add(this);
  }
};

const functionItem = { type: 'function_call', id: 'fc_audit', call_id: 'call_audit', name: 'read', arguments: '{"path":"hello.txt"}', status: 'completed' };
const messageItem = { type: 'message', id: 'msg_audit', role: 'assistant', status: 'completed', content: [{ type: 'output_text', text: '已读取合成文件。', annotations: [] }] };
function eventsFor(turn) {
  const item = turn === 0 ? functionItem : messageItem;
  return [
    { type: 'response.output_item.added', output_index: 0, item },
    { type: 'response.output_item.done', output_index: 0, item },
    { type: 'response.completed', response: {
    id: `resp_audit_${turn}`, object: 'response', model: 'gpt-5.5', status: 'completed',
    output: [item], usage: { input_tokens: 10, output_tokens: 5, total_tokens: 15 },
  }}];
}
function serverFrame(opcode, payload) {
  const body = Buffer.from(payload);
  const head = body.length < 126 ? Buffer.from([0x80 | opcode, body.length]) : Buffer.from([0x80 | opcode, 126, body.length >> 8, body.length & 255]);
  return Buffer.concat([head, body]);
}
// 本地 WS server 只实现测试所需的文本、Ping 和 Close 帧，不协商压缩。
let wsCapture;
const server = createServer((_request, response) => { response.writeHead(400).end(); });
server.on('upgrade', (request, socket, head) => {
  const accept = createHash('sha1').update(request.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
  socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  let buffered = head;
  socket.on('data', data => {
    buffered = Buffer.concat([buffered, data]);
    while (buffered.length >= 2) {
      const opcode = buffered[0] & 15;
      assert(buffered[0] & 128, 'Fragmented frames are not part of this fixture');
      assert(buffered[1] & 128, 'Client frames must be masked');
      let length = buffered[1] & 127;
      let offset = 2;
      if (length === 126) { if (buffered.length < 4) return; length = buffered.readUInt16BE(2); offset = 4; }
      if (length === 127) { if (buffered.length < 10) return; length = Number(buffered.readBigUInt64BE(2)); offset = 10; }
      if (buffered.length < offset + 4 + length) return;
      const mask = buffered.subarray(offset, offset + 4);
      const payload = Buffer.from(buffered.subarray(offset + 4, offset + 4 + length));
      for (let i = 0; i < length; i++) payload[i] ^= mask[i % 4];
      buffered = buffered.subarray(offset + 4 + length);
      if (opcode === 8) { socket.end(serverFrame(8, payload)); return; }
      if (opcode === 9) { socket.write(serverFrame(10, payload)); continue; }
      assert.equal(opcode, 1);
      const pairs = [];
      for (let i = 0; i < request.rawHeaders.length; i += 2) pairs.push([request.rawHeaders[i].toLowerCase(), request.rawHeaders[i + 1]]);
      const turn = wsCapture.length;
      wsCapture.push({ headers: normalizeHeaders(pairs), body: JSON.parse(payload), response_events: eventsFor(turn) });
      for (const event of eventsFor(turn)) socket.write(serverFrame(1, JSON.stringify(event)));
    }
  });
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const baseUrl = `http://127.0.0.1:${server.address().port}`;
allowedWsUrl = `${baseUrl.replace('http:', 'ws:')}/codex/responses`;
function normalizeHeaders(pairs) {
  // 仅替换传输随机值；业务头、SDK 版本和正文不由手工重建。
  return pairs.map(([name, value]) => [name, name === 'host' ? '127.0.0.1:1' : name === 'sec-websocket-key' ? 'c3ludGhldGljLWtleS0xNg==' : value]);
}
const samples = [];
try {
  for (const [api, transport] of [['openai-responses', 'sse'], ['openai-codex-responses', 'sse'], ['openai-codex-responses', 'websocket']]) {
    for (const promptKind of ['default', 'custom']) {
      const captured = [];
      wsCapture = captured;
      const promptOptions = { cwd: '/synthetic/workspace', selectedTools: ['read'], toolSnippets: { read: 'Read a file' }, contextFiles: [], skills: [] };
      if (promptKind === 'custom') promptOptions.customPrompt = 'You are a coding assistant. Use the provided tools accurately, preserve user changes, and report verification results honestly.';
      const systemPrompt = buildSystemPrompt(promptOptions);
      assert.equal(systemPrompt.includes('operating inside pi'), promptKind === 'default');
      const model = { id: 'gpt-5.5', name: 'audit', provider: api === 'openai-responses' ? 'audit-proxy' : 'openai-codex', api, baseUrl, reasoning: true, input: ['text'], cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 }, contextWindow: 100000, maxTokens: 1000 };
      const context = { systemPrompt, messages: [{ role: 'user', content: '请读取 hello.txt，这是合成测试。', timestamp: 0 }], tools: [{ name: 'read', description: 'Read a synthetic file', parameters: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'], additionalProperties: false } }] };
      const jwt = ['synthetic', Buffer.from(JSON.stringify({ 'https://api.openai.com/auth': { chatgpt_account_id: 'synthetic-account' } })).toString('base64url'), 'synthetic'].join('.');
      const sessionId = `synthetic-${api}-${transport}-${promptKind}`;
      const options = {
        apiKey: api === 'openai-responses' ? 'synthetic-client-key' : jwt,
        sessionId, transport, env: {}, maxRetries: 0, timeoutMs: 10000,
        fetch: async (input, init) => {
          assert.equal(transport, 'sse', 'WS must not silently fall back to HTTP');
          const request = new Request(input, init);
          assert.equal(new URL(request.url).origin, baseUrl);
          let body = Buffer.from(await request.arrayBuffer());
          const originalEncoding = request.headers.get('content-encoding');
          if (originalEncoding === 'zstd') body = zstdDecompressSync(body);
          const turn = captured.length;
          captured.push({ headers: normalizeHeaders([...request.headers]), body: JSON.parse(body), content_encoding: originalEncoding, response_events: eventsFor(turn) });
          return new Response(eventsFor(turn).map(e => `event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`).join(''), { status: 200, headers: { 'content-type': 'text/event-stream' } });
        },
      };
      const provider = api === 'openai-responses' ? responses : codex;
      for (let turn = 0; turn < 2; turn++) {
        let completed;
        for await (const event of provider.stream(model, context, options)) {
          assert.notEqual(event.type, 'error', event.error?.errorMessage);
          if (event.type === 'done') completed = event.message;
        }
        assert(completed, 'Provider must deliver a terminal response');
        if (turn === 0) {
          const call = completed.content.find(item => item.type === 'toolCall');
          assert.equal(call?.name, 'read');
          context.messages.push(completed, { role: 'toolResult', toolCallId: call.id, toolName: 'read', content: [{ type: 'text', text: '合成文件内容：hello pi' }], isError: false, timestamp: 0 });
        } else assert(completed.content.some(item => item.type === 'text' && item.text === '已读取合成文件。'));
      }
      assert.equal(captured.length, 2);
      codex.closeOpenAICodexWebSocketSessions(sessionId);
      samples.push({ name: `${api}-${transport}-${promptKind}`, api, provider: model.provider, transport, prompt_kind: promptKind, turns: captured });
    }
  }
  const sha256 = async path => createHash('sha256').update(await readFile(path)).digest('hex');
  const fixture = { source: {
    pi_ai: aiPackage.version, pi_agent: agentPackage.version,
    openai_sdk: samples[0].turns[0].headers.find(([name]) => name === 'x-stainless-package-version')[1],
    node: process.version,
    websocket_runtime: `undici/${requireAgent('undici/package.json').version}`,
    generator: 'capture_pi.mjs', network: 'mock fetch and loopback WS only',
    responses_sha256: await sha256(`${ai}/dist/api/openai-responses.js`),
    codex_responses_sha256: await sha256(`${ai}/dist/api/openai-codex-responses.js`),
    system_prompt_sha256: await sha256(`${agent}/dist/core/system-prompt.js`),
    normalization: 'HTTP bodies decoded from zstd; host port and WebSocket key replaced; PI_PACKAGE_DIR and cwd are synthetic',
  }, samples };
  await writeFile(outputPath, JSON.stringify(fixture, null, 2) + '\n');
  console.log(`Captured ${samples.length} cases / 12 turns with successful tool and terminal delivery.`);
} finally {
  codex.closeOpenAICodexWebSocketSessions();
  for (const socket of sockets) socket.close();
  await new Promise(resolve => server.close(resolve));
  await dispatcher.close();
}
