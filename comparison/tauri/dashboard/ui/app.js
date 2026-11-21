const DATA = {
  7: {
    stats: [
      ["Revenue", "$48,210", "+6.2%", true],
      ["Active users", "12,480", "+3.1%", true],
      ["Conversion", "3.4%", "+0.2 pts", true],
      ["p95 latency", "182 ms", "+9 ms", false],
    ],
    bars: [5.1, 6.4, 7.2, 6.8, 8.9, 7.7, 6.1],
    labels: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
  },
  30: {
    stats: [
      ["Revenue", "$201,940", "+11.8%", true],
      ["Active users", "41,206", "+7.4%", true],
      ["Conversion", "3.1%", "-0.1 pts", false],
      ["p95 latency", "171 ms", "-12 ms", true],
    ],
    bars: [14.2, 16.8, 15.1, 18.4, 17.0, 19.6, 21.2, 20.1, 22.8, 24.3],
    labels: ["1", "4", "7", "10", "13", "16", "19", "22", "25", "28"],
  },
  90: {
    stats: [
      ["Revenue", "$587,330", "+18.5%", true],
      ["Active users", "96,812", "+14.0%", true],
      ["Conversion", "2.9%", "+0.4 pts", true],
      ["p95 latency", "166 ms", "-21 ms", true],
    ],
    bars: [38, 41, 44, 40, 47, 52, 49, 55, 58, 54, 61, 66],
    labels: ["W1", "W2", "W3", "W4", "W5", "W6", "W7", "W8", "W9", "W10", "W11", "W12"],
  },
};

const ORDERS = [
  ["Northwind Traders", "Business", "$1,200.00", "Paid"],
  ["Ada Okafor", "Pro", "$49.00", "Paid"],
  ["Globex Corp", "Enterprise", "$4,800.00", "Pending"],
  ["Lena Fischer", "Pro", "$49.00", "Refunded"],
  ["Initech", "Business", "$1,200.00", "Paid"],
];

const $ = (id) => document.getElementById(id);

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function render(range) {
  const { stats, bars, labels } = DATA[range];

  $("tiles").replaceChildren(...stats.map(([label, value, delta, good]) => {
    const tile = el("div", "tile");
    // "good" is supplied per stat: for latency a rise is bad, so it can't be inferred from the sign.
    tile.append(el("div", "label", label), el("div", "value", value), el("div", good ? "delta" : "delta down", delta));
    return tile;
  }));

  const max = Math.max(...bars);
  $("chart").replaceChildren(...bars.map((v, i) => {
    const bar = el("div", "bar");
    const fill = el("div", "fill");
    fill.style.height = `${(v / max) * 100}%`;
    fill.title = `$${v}k`;
    bar.append(fill, el("span", "tick", labels[i]));
    return bar;
  }));

  document.querySelectorAll("#range button").forEach((b) =>
    b.setAttribute("aria-pressed", String(b.dataset.range === String(range))));
}

$("orders").replaceChildren(...ORDERS.map(([customer, plan, amount, status]) => {
  const tr = el("tr");
  tr.append(el("td", "", customer), el("td", "", plan), el("td", "num", amount), el("td", `status-${status.toLowerCase()}`, status));
  return tr;
}));

$("range").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (b) render(b.dataset.range);
});

$("theme").addEventListener("click", () => {
  const dark = document.documentElement.dataset.theme !== "dark";
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  $("theme").textContent = dark ? "Light" : "Dark";
});

render(7);
