const seed = [
  { title: "Welcome to Notes", body: "A quiet place for short thoughts. Pick a note on the left, or start a new one.", pinned: true },
  { title: "Groceries", body: "Oat milk, coffee beans, lemons, sourdough, olive oil.\nCheck whether we still have rice.", pinned: false },
  { title: "Meeting with Priya", body: "Agenda: roadmap for Q3, hiring plan, design review cadence.\nFollow up on the pricing experiment.", pinned: true },
  { title: "Book list", body: "The Pragmatic Programmer\nDesigning Data-Intensive Applications\nThe Design of Everyday Things", pinned: false },
  { title: "Trip ideas", body: "Lisbon in October. Train to Porto for a day.\nBook a table somewhere with a view.", pinned: false },
];

let nextId = 1;
const notes = seed.map((n) => ({ id: nextId++, ...n }));
let selectedId = notes[0].id;

const $ = (id) => document.getElementById(id);
const els = {
  search: $("search"), list: $("list"), title: $("title"), body: $("body"),
  count: $("count"), pin: $("pin"), del: $("delete"), theme: $("theme"),
  editor: $("editor"), empty: $("empty"),
};

const selected = () => notes.find((n) => n.id === selectedId);
const firstLine = (text) => text.split("\n").find((l) => l.trim()) ?? "";
const wordCount = (text) => text.trim() ? text.trim().split(/\s+/).length : 0;

function visibleNotes() {
  const q = els.search.value.trim().toLowerCase();
  return notes
    .filter((n) => !q || (n.title + "\n" + n.body).toLowerCase().includes(q))
    .sort((a, b) => Number(b.pinned) - Number(a.pinned)); // stable: keeps creation order within groups
}

function renderList() {
  const items = visibleNotes();
  els.list.replaceChildren();
  if (!items.length) {
    const li = document.createElement("li");
    li.className = "no-results";
    li.textContent = "No matching notes";
    els.list.append(li);
    return;
  }
  for (const n of items) {
    const li = document.createElement("li");
    const btn = document.createElement("button");
    btn.dataset.id = n.id;
    if (n.id === selectedId) btn.setAttribute("aria-current", "true");
    if (n.pinned) btn.append(Object.assign(document.createElement("div"), { className: "pinned-tag", textContent: "Pinned" }));
    btn.append(
      Object.assign(document.createElement("div"), { className: "note-title", textContent: n.title || "Untitled" }),
      Object.assign(document.createElement("div"), { className: "note-preview", textContent: firstLine(n.body) || "No content" }),
    );
    li.append(btn);
    els.list.append(li);
  }
}

function renderEditor() {
  const n = selected();
  els.editor.hidden = !n;
  els.empty.hidden = !!n;
  if (!n) return;
  els.title.value = n.title;
  els.body.value = n.body;
  els.pin.textContent = n.pinned ? "Unpin" : "Pin";
  renderCount();
}

function renderCount() {
  const w = wordCount(els.body.value);
  els.count.textContent = `${w} ${w === 1 ? "word" : "words"}`;
}

els.list.addEventListener("click", (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  selectedId = Number(btn.dataset.id);
  renderList();
  renderEditor();
});

els.search.addEventListener("input", renderList);

$("new").addEventListener("click", () => {
  const n = { id: nextId++, title: "", body: "", pinned: false };
  notes.unshift(n);
  selectedId = n.id;
  els.search.value = "";
  renderList();
  renderEditor();
  els.title.focus();
});

els.title.addEventListener("input", () => { selected().title = els.title.value; renderList(); });
els.body.addEventListener("input", () => { selected().body = els.body.value; renderCount(); renderList(); });

els.pin.addEventListener("click", () => {
  const n = selected();
  n.pinned = !n.pinned;
  renderList();
  renderEditor();
});

els.del.addEventListener("click", () => {
  const i = notes.findIndex((n) => n.id === selectedId);
  notes.splice(i, 1);
  selectedId = (notes[i] ?? notes[i - 1])?.id;
  renderList();
  renderEditor();
});

els.theme.addEventListener("click", () => {
  const dark = document.documentElement.dataset.theme !== "dark";
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  els.theme.textContent = dark ? "Light" : "Dark";
});

renderList();
renderEditor();
