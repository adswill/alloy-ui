// Checks the extension without VS Code: the manifest is consistent, `activate` wires its
// commands (against a stub `vscode`), and, when an `alloy` binary is available, the real
// language server answers a scripted session over JSON-RPC.
'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const Module = require('module');
const { spawn } = require('child_process');

const pkg = require('../package.json');

// ---- manifest -------------------------------------------------------------------------
const declared = pkg.contributes.commands.map((c) => c.command);
for (const c of ['alloy.dev', 'alloy.test', 'alloy.restartServer']) {
  assert(declared.includes(c), `command ${c} is declared`);
}
assert.strictEqual(pkg.main, './extension.js');
assert(fs.existsSync(path.join(__dirname, '..', pkg.main)), 'main file exists');
assert(pkg.dependencies['vscode-languageclient'], 'language client is a runtime dependency');
assert(pkg.contributes.configuration.properties['alloy.server.path']);
for (const g of pkg.contributes.grammars) assert(fs.existsSync(path.join(__dirname, '..', g.path)));
for (const s of pkg.contributes.snippets) assert(fs.existsSync(path.join(__dirname, '..', s.path)));

// ---- activate against stubs -----------------------------------------------------------
const registered = new Map();
const started = [];
const terminals = [];
const vscodeStub = {
  workspace: {
    getConfiguration: () => ({ get: (k) => (k === 'server.path' ? 'alloy-test-bin' : undefined) }),
    onDidChangeConfiguration: () => ({ dispose() {} }),
    workspaceFolders: [{ uri: 'file:///proj' }],
  },
  commands: {
    registerCommand: (id, fn) => (registered.set(id, fn), { dispose() {} }),
    executeCommand: async (id) => registered.get(id)(),
  },
  window: {
    createTerminal: (o) => {
      const t = { o, sent: [], show() {}, sendText(s) { this.sent.push(s); } };
      terminals.push(t);
      return t;
    },
    showWarningMessage: async () => undefined,
  },
  env: { clipboard: { writeText: async () => {} } },
};
class StubClient {
  constructor(id, name, server, options) {
    this.server = server;
    this.options = options;
    started.push(this);
  }
  async start() {}
  async stop() { this.stopped = true; }
}
const realLoad = Module._load;
Module._load = function (request, ...rest) {
  if (request === 'vscode') return vscodeStub;
  if (request === 'vscode-languageclient/node') return { LanguageClient: StubClient };
  return realLoad.call(this, request, ...rest);
};
const ext = require('../extension.js');

async function activation() {
  await ext.activate({ subscriptions: [] });
  assert.strictEqual(started.length, 1);
  assert.deepStrictEqual(started[0].server, { command: 'alloy-test-bin', args: ['lsp'] });
  assert.strictEqual(started[0].options.documentSelector[0].language, 'alloy');
  registered.get('alloy.dev')();
  registered.get('alloy.test')();
  assert.deepStrictEqual(terminals.map((t) => t.sent[0]), ['alloy-test-bin dev', 'alloy-test-bin test']);
  await registered.get('alloy.restartServer')();
  assert.strictEqual(started.length, 2, 'restart starts a new client');
  assert(started[0].stopped, 'and stops the old one');
  await ext.deactivate();
  Module._load = realLoad;
}

// ---- the real server ------------------------------------------------------------------
function findBinary() {
  if (process.env.ALLOY_BIN) return process.env.ALLOY_BIN;
  const exe = process.platform === 'win32' ? 'alloy.exe' : 'alloy';
  for (const profile of ['debug', 'release']) {
    const p = path.join(__dirname, '..', '..', '..', 'target', profile, exe);
    if (fs.existsSync(p)) return p;
  }
  return null;
}

async function server(bin) {
  const rpc = require('vscode-jsonrpc/node');
  const child = spawn(bin, ['lsp'], { stdio: ['pipe', 'pipe', 'inherit'] });
  const conn = rpc.createMessageConnection(
    new rpc.StreamMessageReader(child.stdout),
    new rpc.StreamMessageWriter(child.stdin)
  );
  const diagnostics = [];
  conn.onNotification('textDocument/publishDiagnostics', (p) => diagnostics.push(p));
  conn.listen();
  const uri = 'file:///ui/home.alloy';

  const init = await conn.sendRequest('initialize', { processId: null, rootUri: null, capabilities: {} });
  assert.strictEqual(init.capabilities.hoverProvider, true);
  await conn.sendNotification('initialized', {});
  await conn.sendNotification('textDocument/didOpen', {
    textDocument: { uri, languageId: 'alloy', version: 1, text: 'col page\n  bttn "Add"\n  button "Add" primary @add\n  \n' },
  });
  await waitFor(() => diagnostics.length > 0, 'diagnostics');
  const first = diagnostics[0].diagnostics;
  assert.strictEqual(first.length, 1);
  assert(first[0].message.startsWith('unknown element `bttn`'), first[0].message);
  assert.strictEqual(first[0].severity, 1);

  const items = await conn.sendRequest('textDocument/completion', {
    textDocument: { uri }, position: { line: 3, character: 2 },
  });
  const labels = items.map((i) => i.label);
  assert(labels.includes('button') && labels.includes('row'), labels.join());
  const hover = await conn.sendRequest('textDocument/hover', {
    textDocument: { uri }, position: { line: 2, character: 4 },
  });
  assert(hover.contents.value.includes('button'));

  await conn.sendNotification('textDocument/didChange', {
    textDocument: { uri, version: 2 },
    contentChanges: [{ text: 'col page\n  button "Add" primary @add\n' }],
  });
  await waitFor(() => diagnostics.length > 1, 'diagnostics after fix');
  assert.strictEqual(diagnostics[1].diagnostics.length, 0);

  await conn.sendRequest('shutdown');
  await conn.sendNotification('exit');
  const code = await new Promise((r) => child.on('exit', r));
  assert.strictEqual(code, 0, 'clean exit after shutdown');
  conn.dispose();
}

async function waitFor(cond, what) {
  for (let i = 0; i < 100; i++) {
    if (cond()) return;
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(`timed out waiting for ${what}`);
}

(async () => {
  await activation();
  const bin = findBinary();
  if (bin) {
    await server(bin);
    console.log(`extension.test.js: manifest, activation and language server (${bin}) ok`);
  } else if (process.env.CI) {
    throw new Error('CI must build the alloy binary first (set ALLOY_BIN or build target/debug/alloy)');
  } else {
    console.log('extension.test.js: manifest and activation ok; server skipped (no alloy binary; run `cargo build -p alloy-ui-cli`)');
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
