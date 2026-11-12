// The panel page. Everything the server sends is shown with textContent, never as HTML.
const $ = (id) => document.getElementById(id);
// ?zoom=0.6 shrinks the page (for a small window, or a screenshot).
const zoom = Number(new URLSearchParams(location.search).get("zoom"));
if (zoom > 0.2 && zoom < 3) document.body.style.zoom = String(zoom);
let state = null;

async function post(action, body = {}) {
  const res = await fetch(`/api/${action}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error ?? res.statusText);
}
const act = (action, body) => post(action, body).catch((e) => show({ ...state, message: e.message, messageIsError: true }));

const WAYS = { N: "↑ northbound", S: "↓ southbound", E: "→ eastbound", W: "← westbound", fwd: "↑ along the line", back: "↓ against the line" };
const rgb = (f) => `rgb(${f.map((c) => Math.round(c * 255)).join(",")})`;

function el(tag, props = {}, ...kids) {
  const e = Object.assign(document.createElement(tag), props);
  for (const k of kids) e.append(k);
  return e;
}

function drawItems(list, bar, total, view, editable) {
  list.replaceChildren();
  bar.replaceChildren();
  const sum = view.total_width || 1;
  for (const it of view.items) {
    bar.append(el("i", { title: `${it.label} ${it.width} m`, style: `width:${(it.width / sum) * 100}%;background:${it.fill ? rgb(it.fill) : "#ccc"}` }));
    const li = el("li", {}, el("span", { className: "sw", style: `background:${it.fill ? rgb(it.fill) : "#ccc"}` }), el("span", { className: "name" }, it.label));
    if (it.way) li.append(el("span", { className: "way" }, WAYS[it.way] ?? it.way));
    if (editable) {
      const w = el("button", { className: `w${it.explicit_width ? " fixed" : ""}`, title: "Change this width (metres)" }, `${it.width} m`);
      w.onclick = () => {
        const v = prompt(`Width of "${it.label}" in metres`, String(it.width));
        if (v !== null && Number(v) > 0) act("edit", { op: ["width", it.index, Number(v)] });
      };
      li.append(w);
      const mv = (label, title, to) => {
        const b = el("button", { className: "plain tiny", title }, label);
        b.disabled = to < 0 || to >= view.items.length;
        b.onclick = () => act("edit", { op: ["move", it.index, to] });
        return b;
      };
      const del = el("button", { className: "plain tiny", title: `Remove ${it.label}` }, "✕");
      del.disabled = view.items.length < 2;
      del.onclick = () => act("edit", { op: ["delete", it.index] });
      li.append(mv("◀", "Move left", it.index - 1), mv("▶", "Move right", it.index + 1), del);
    } else {
      li.append(el("span", { className: "w" }, `${it.width} m`));
    }
    list.append(li);
  }
  total.textContent = `${view.items.length} elements, ${view.total_width} m wide${view.facing ? `, read facing ${view.facing}` : ""}`;
}

function show(s) {
  state = s;
  $("dot").classList.toggle("on", !!s.connected);
  $("problem").hidden = !s.error;
  $("problem").textContent = s.error ?? "";

  const agents = $("agent");
  if (agents.options.length !== s.agents.list.length) agents.replaceChildren(...s.agents.list.map((a) => el("option", { value: a.id, textContent: a.label })));
  if (document.activeElement !== agents) agents.value = s.agents.default;

  const t = s.target;
  const ctx = $("context");
  ctx.replaceChildren();
  if (!s.doc) ctx.append(el("p", { className: "muted" }, "Waiting for VectorCraft…"));
  else if (!t) ctx.append(el("strong", {}, "Nothing selected"), el("p", { className: "muted" }, "Draw a line in VectorCraft and select it, or select a road, then describe it here."));
  else if (t.kind === "line") ctx.append(el("strong", {}, "A plain line is selected"), el("p", { className: "muted" }, "Describe it below to turn it into a road."));
  else if (t.kind === "road") ctx.append(el("strong", {}, `Road ${s.road.segment}`), el("p", { className: "muted" }, s.road.described ? `${s.road.pieces.selected} of ${s.road.pieces.total} piece${s.road.pieces.total === 1 ? "" : "s"} selected` : "No description yet (drawn as two lanes)"));
  else ctx.append(el("strong", {}, "Select one line or one road"), el("p", { className: "muted" }, "The selection holds something this panel does not describe."));

  $("road").hidden = !s.road;
  if (s.road) drawItems($("items"), $("bar"), $("total"), s.road, !s.pending);

  $("proposal").hidden = !s.pending;
  $("asked").textContent = s.pending?.description ? `You asked: “${s.pending.description}”` : "";
  $("asked").hidden = !s.pending?.description;
  if (s.pending) {
    drawItems($("pitems"), $("pbar"), $("ptotal"), s.pending, true);
    $("shorthand").value = s.pending.canonical;
  } else if (s.road && document.activeElement !== $("shorthand")) $("shorthand").value = s.road.text;

  const scope = $("scope");
  const multi = s.road && s.road.pieces.total > s.road.pieces.selected;
  scope.hidden = !multi;
  if (multi) $("scope-all").textContent = `All ${s.road.pieces.total} pieces of this road`;

  const canAct = !!t && t.kind !== "other" && !s.busy;
  $("propose").disabled = !canAct || !$("words").value.trim();
  $("draw").disabled = !canAct;
  $("draw").textContent = s.busy === "drawing" ? "Drawing…" : s.pending ? "Draw this" : "Redraw";
  $("thinking").hidden = s.busy !== "thinking";
  const msg = $("message");
  msg.textContent = s.message ?? "";
  msg.classList.toggle("err", !!s.messageIsError || /could not|cannot|unknown|select /i.test(s.message ?? ""));
}

$("propose").onclick = () => act("propose", { description: $("words").value.trim(), agent: $("agent").value });
$("words").oninput = () => state && show(state);
$("words").onkeydown = (e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && !$("propose").disabled) $("propose").click(); };
$("draw").onclick = () => act("draw", { scope: document.querySelector("input[name=scope]:checked")?.value });
$("cancel").onclick = () => act("cancel");
$("use").onclick = () => act("text", { text: $("shorthand").value });
$("agent").onchange = () => act("agent", { id: $("agent").value });

const events = new EventSource("/events");
events.onmessage = (m) => show(JSON.parse(m.data));
events.onerror = () => $("dot").classList.remove("on");
