// Tokenises sample .alloy text with the real TextMate engine VS Code uses and checks scopes.
// Run: npm install && npm test   (dev dependencies only; nothing ships in the extension)
const fs = require("fs"), path = require("path"), assert = require("assert");
const vsctm = require("vscode-textmate"), oniguruma = require("vscode-oniguruma");

const wasm = fs.readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm")).buffer;
const lib = oniguruma.loadWASM(wasm).then(() => ({
  createOnigScanner: (p) => new oniguruma.OnigScanner(p),
  createOnigString: (s) => new oniguruma.OnigString(s),
}));
const registry = new vsctm.Registry({
  onigLib: lib,
  loadGrammar: async (scope) => scope === "source.alloy"
    ? vsctm.parseRawGrammar(fs.readFileSync(path.join(__dirname, "../syntaxes/alloy.tmLanguage.json"), "utf8"), "alloy.json")
    : null,
});

function scopesAt(line, tokens, needle) {
  const i = line.indexOf(needle);
  const t = tokens.find((t) => t.startIndex <= i && i < t.endIndex);
  return t ? t.scopes : [];
}

(async () => {
  const g = await registry.loadGrammar("source.alloy");
  let stack = vsctm.INITIAL;
  const tok = (line) => { const r = g.tokenizeLine(line, stack); stack = r.ruleStack; return r.tokens; };
  const has = (line, needle, scope) => {
    const sc = scopesAt(line, tok(line), needle);
    assert.ok(sc.some((s) => s.startsWith(scope)), `${JSON.stringify(needle)} in ${JSON.stringify(line)} -> ${sc.join(" ")}, wanted ${scope}`);
  };

  has("# Edit and save this file", "Edit", "comment.line");
  has("theme accent=#6D4AFF", "theme", "keyword.control");
  has("theme accent=#6D4AFF", "accent", "entity.other.attribute-name");
  has("theme accent=#6D4AFF", "#6D4AFF", "constant.other.color");
  has("theme dark density=compact", "dark", "support.constant.flag");
  has("theme dark density=compact", "compact", "constant.language");
  has("col page gap=14", "col", "entity.name.tag");
  has("col page gap=14", "page", "support.constant.flag");
  has("col page gap=14", "gap", "entity.other.attribute-name");
  has("col page gap=14", "14", "constant.numeric");
  has('  heading "Hello, {name}"', "heading", "entity.name.tag");
  has('  heading "Hello, {name}"', "Hello", "string.quoted");
  has('  heading "Hello, {name}"', "name", "variable.other.interpolated");
  has('  text "{n} so far" dim', "dim", "support.constant.flag");
  has('    button "Add" primary @add', "primary", "support.constant.flag");
  has('    button "Add" primary @add', "add", "entity.name.function.action");
  has("  bttn \"Oops\"", "bttn", "invalid.illegal");
  // a word inside a string must stay a string, not become a flag
  has('  text "this is dim and primary"', "primary", "string.quoted");
  console.log("grammar: all scope checks passed");
})().catch((e) => { console.error(e.message); process.exit(1); });
