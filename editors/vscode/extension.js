// Alloy for VS Code: starts `alloy lsp` for .alloy live views (diagnostics, completion, hover)
// and adds two commands that run `alloy dev` and `alloy test` in a terminal.
'use strict';

const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');

/** @type {LanguageClient | undefined} */
let client;

const INSTALL = 'cargo install --git https://github.com/adswill/alloy-ui alloy-ui-cli';

function serverPath() {
  return vscode.workspace.getConfiguration('alloy').get('server.path') || 'alloy';
}

async function startServer(context) {
  const command = serverPath();
  client = new LanguageClient(
    'alloy',
    'Alloy language server',
    { command, args: ['lsp'] },
    { documentSelector: [{ scheme: 'file', language: 'alloy' }, { scheme: 'untitled', language: 'alloy' }] }
  );
  try {
    await client.start();
  } catch (err) {
    client = undefined;
    const pick = await vscode.window.showWarningMessage(
      `Alloy: could not start "${command} lsp" (${err.message}). Syntax colours and snippets still work; ` +
        `install the tool for diagnostics and completion.`,
      'Copy install command',
      'Set path'
    );
    if (pick === 'Copy install command') {
      await vscode.env.clipboard.writeText(INSTALL);
    } else if (pick === 'Set path') {
      await vscode.commands.executeCommand('workbench.action.openSettings', 'alloy.server.path');
    }
  }
}

function runInTerminal(name, commandLine) {
  const folder = vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders[0];
  const terminal = vscode.window.createTerminal({ name, cwd: folder && folder.uri });
  terminal.show();
  terminal.sendText(commandLine);
}

async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand('alloy.dev', () => runInTerminal('Alloy dev', `${serverPath()} dev`)),
    vscode.commands.registerCommand('alloy.test', () => runInTerminal('Alloy test', `${serverPath()} test`)),
    vscode.commands.registerCommand('alloy.restartServer', async () => {
      if (client) {
        await client.stop();
      }
      await startServer(context);
    }),
    vscode.workspace.onDidChangeConfiguration(async (e) => {
      if (e.affectsConfiguration('alloy.server.path')) {
        await vscode.commands.executeCommand('alloy.restartServer');
      }
    })
  );
  await startServer(context);
}

async function deactivate() {
  if (client) {
    await client.stop();
  }
}

module.exports = { activate, deactivate };
