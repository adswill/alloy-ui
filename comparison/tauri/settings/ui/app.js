const $ = (id) => document.getElementById(id);
const root = document.documentElement;

// Sidebar navigation
$("nav").addEventListener("click", (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  for (const b of $("nav").children) {
    if (b === btn) b.setAttribute("aria-current", "page");
    else b.removeAttribute("aria-current");
  }
  document.querySelectorAll(".page").forEach((p) => { p.hidden = p.id !== btn.dataset.page; });
});

// Theme and accent: the accent has a lighter variant for dark mode, so re-apply on either change.
let theme = "light";
let accent = $("accent").querySelector("[aria-checked=true]");

function applyAppearance() {
  root.dataset.theme = theme;
  root.style.setProperty("--accent", accent.dataset[theme]);
}

$("theme").addEventListener("click", (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  theme = btn.dataset.theme;
  for (const b of $("theme").children) b.setAttribute("aria-pressed", String(b === btn));
  applyAppearance();
});

$("accent").addEventListener("click", (e) => {
  const swatch = e.target.closest(".swatch");
  if (!swatch) return;
  accent = swatch;
  for (const s of $("accent").children) s.setAttribute("aria-checked", String(s === swatch));
  applyAppearance();
});

$("accent").querySelectorAll(".swatch").forEach((s) => {
  s.style.background = s.dataset.light;
});

// Sliders
$("size").addEventListener("input", (e) => {
  root.style.setProperty("--text-size", `${e.target.value}px`);
  $("size-value").textContent = `${e.target.value} px`;
});
$("volume").addEventListener("input", (e) => {
  $("volume-value").textContent = `${e.target.value}%`;
});

// Clear history confirmation
const dialog = $("confirm");
$("clear").addEventListener("click", () => dialog.showModal());
dialog.addEventListener("close", () => {
  if (dialog.returnValue === "clear") $("clear-status").textContent = "History cleared.";
});
