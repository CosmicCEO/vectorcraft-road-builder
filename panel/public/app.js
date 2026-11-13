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

function fillList(id, items) {
  const ul = $(id);
  ul.replaceChildren(...(items ?? []).map((t) => el("li", {}, t)));
  ul.hidden = !items?.length;
}

function show(s) {
  state = s;
  $("notesCard").hidden = !s.notes?.length;
  $("notes").replaceChildren(...(s.notes ?? []).map((n) => el("li", {}, n)));
  $("dot").classList.toggle("on", !!s.connected);
  $("problem").hidden = !s.error;
  $("problem").textContent = s.error ?? "";

  const agents = $("agent");
  if (agents.options.length !== s.agents.list.length) agents.replaceChildren(...s.agents.list.map((a) => el("option", { value: a.id, textContent: a.label })));
  if (document.activeElement !== agents) agents.value = s.agents.default;

  const t = s.target;
  const pj = s.pending?.kind === "junction" ? s.pending : null; // a proposed junction
  const ctx = $("context");
  ctx.replaceChildren();
  if (!s.doc) ctx.append(el("p", { className: "muted" }, "Waiting for VectorCraft…"));
  else if (!t) ctx.append(el("strong", {}, "Nothing selected"), el("p", { className: "muted" }, "Draw a line in VectorCraft and select it, or select a road, then describe it here."));
  else if (t.kind === "line") ctx.append(el("strong", {}, "A plain line is selected"), el("p", { className: "muted" }, s.mode === "connection" ? "Describe the junction below, or just carry the road across." : "Describe it below to turn it into a road."));
  else if (t.kind === "road") ctx.append(el("strong", {}, `Road ${s.road.segment}`), el("p", { className: "muted" }, s.road.described ? `${s.road.pieces.selected} of ${s.road.pieces.total} piece${s.road.pieces.total === 1 ? "" : "s"} selected` : "No description yet (drawn as two lanes)"));
  else if (t.kind === "junction") ctx.append(el("strong", {}, `Junction ${s.junction.id}`), el("p", { className: "muted" }, s.junction.described ? "A connecting line with a description" : "A connecting line, not described yet"));
  else if (t.kind === "hub") ctx.append(el("strong", {}, `${t.roads.length} roads meet: ${t.roads.join(", ")}`), el("p", { className: "muted" }, s.hub?.described ? "They form an intersection. Change it below, then Draw." : "Choose what happens where they meet, then Draw."));
  else if (t.kind === "crossing") ctx.append(el("strong", {}, `Roads ${t.roads[0]} and ${t.roads[1]} cross`), el("p", { className: "muted" }, s.crossing?.described ? "They form an intersection. Change it below, then Draw." : "Choose what happens where they cross, then Draw."));
  else ctx.append(el("strong", {}, "Select one line or one road"), el("p", { className: "muted" }, "The selection holds something this panel does not describe."));
  if (t && (t.kind === "road" || t.kind === "junction") && !s.busy) {
    const b = el("button", { className: "plain", title: "Select the road's own line in VectorCraft, to move or reshape it" }, "Select the line");
    b.onclick = () => act("selectline");
    ctx.append(el("div", { className: "row" }, b, el("span", { className: "muted" }, "Click anywhere on a road to select it here.")));
  }

  // A plain line can be a road or a connection between two roads.
  $("modeCard").hidden = t?.kind !== "line";
  for (const r of document.querySelectorAll("input[name=mode]")) r.checked = r.value === s.mode;
  $("matchRow").hidden = s.mode !== "connection";

  // Crossing card: the proposed intersection, or the stored one, or the starting suggestion for two selected roads.
  const pc = s.pending?.kind === "crossing" ? s.pending : null;
  const cv = t?.kind === "crossing" && s.crossing ? { spec: pc ? pc.crossing : s.crossing.spec, summary: pc ? pc.summary : s.crossing.summary, proposed: !!pc } : null;
  $("crossingCard").hidden = !cv;
  if (cv) {
    const [ra, rb] = s.crossing.roads;
    const sp = cv.spec;
    const tr = sp.treatment;
    $("ctitle").textContent = cv.proposed ? "Proposed intersection" : s.crossing.described ? "Intersection" : "Make an intersection";
    $("casked").textContent = pc?.description ? `You asked: “${pc.description}”` : "";
    $("casked").hidden = !pc?.description;
    $("csummary").textContent = cv.summary;
    fillList("cchanges", pc ? pc.changes : null);
    $("ctreat").value = tr;
    const atGrade = ["signal", "two-way-stop", "four-way-stop", "uncontrolled", "driveway", "on-ramp", "off-ramp"].includes(tr);
    const pick = (id, blank) => {
      const sel = $(id);
      if (sel.options.length !== 3 || sel.options[1].value !== ra || sel.options[2].value !== rb) sel.replaceChildren(el("option", { value: "", textContent: blank }), el("option", { value: ra, textContent: `Road ${ra}` }), el("option", { value: rb, textContent: `Road ${rb}` }));
      return sel;
    };
    pick("cminor", "the narrower road").value = sp.minor ?? "";
    pick("cover", `Road ${ra}`).value = sp.over ?? "";
    pick("cmain", "worked out for me").value = sp.main ?? "";
    const only = (id, on) => ($(id).hidden = !on);
    only("cminorRow", tr === "two-way-stop");
    only("coverRow", tr === "overpass" || tr === "cloverleaf");
    only("cmainRow", ["median-opening", "midblock-crossing", "railroad", "on-ramp", "off-ramp"].includes(tr));
    only("cdiaRow", tr === "roundabout");
    only("copenRow", tr === "median-opening");
    only("caccelRow", tr === "on-ramp" || tr === "off-ramp");
    only("ccrossRow", atGrade && !["driveway", "on-ramp", "off-ramp"].includes(tr) || tr === "roundabout");
    only("cbeaconRow", tr === "midblock-crossing");
    only("cgatesRow", tr === "railroad");
    only("cslipRow", atGrade && !["driveway", "on-ramp", "off-ramp"].includes(tr));
    only("cpocketRow", atGrade && !["driveway", "on-ramp", "off-ramp"].includes(tr));
    only("cstubRow", atGrade);
    $("ccross").checked = !!sp.crosswalks;
    $("cbeacon").checked = !!sp.beacon;
    $("cgates").checked = sp.gates !== false;
    $("cslip").checked = !!sp.slip;
    $("cpocket").value = sp.pocket ?? "none";
    $("cpocketLenRow").hidden = (sp.pocket ?? "none") === "none";
    $("cstub").value = sp.stub ?? "keep";
    for (const [id, v, d] of [["cdia", sp.diameter, 40], ["copen", sp.opening, ""], ["caccel", sp.accel_length, 120], ["cpocketlen", sp.pocket_length, 30]]) if (document.activeElement !== $(id)) $(id).value = v ?? d;
    $("ccancel").hidden = !cv.proposed;
  }

  // Hub card: three or more roads that meet at one point.
  const ph = s.pending?.kind === "hub" ? s.pending : null;
  const hv = t?.kind === "hub" && s.hub ? { spec: ph ? ph.hub : s.hub.spec, summary: ph ? ph.summary : s.hub.summary, proposed: !!ph } : null;
  $("hubCard").hidden = !hv;
  if (hv) {
    $("htitle").textContent = hv.proposed ? "Proposed intersection" : s.hub.described ? "Roads meeting at a point" : "Make an intersection of these roads";
    $("hasked").textContent = ph?.description ? `You asked: “${ph.description}”` : "";
    $("hasked").hidden = !ph?.description;
    $("hsummary").textContent = hv.summary;
    fillList("hchanges", ph ? ph.changes : null);
    $("htreat").value = hv.spec.treatment;
    $("hcross").checked = !!hv.spec.crosswalks;
    $("hstub").value = hv.spec.stub ?? "keep";
    $("hcancel").hidden = !hv.proposed;
  }

  // Junction card: the proposed junction, or the selected junction's description.
  const jv = pj ? { spec: pj.junction, summary: pj.summary, proposed: true } : t?.kind === "junction" && s.junction ? { spec: s.junction.spec, summary: s.junction.summary } : null;
  $("junctionCard").hidden = !jv;
  if (jv) {
    $("jtitle").textContent = jv.proposed ? "Proposed junction" : `Junction ${s.junction.id}`;
    $("jasked").textContent = pj?.description ? `You asked: “${pj.description}”` : "";
    $("jasked").hidden = !pj?.description;
    $("jsummary").textContent = jv.summary;
    fillList("jchanges", pj ? pj.changes : null);
    const legs = $("jlegs");
    legs.replaceChildren();
    jv.spec.legs.forEach((l, i) => {
      const rm = el("button", { className: "plain tiny", title: "Remove this leg" }, "✕");
      rm.onclick = () => act("junction", { patch: { removeLeg: i } });
      legs.append(el("li", { className: "legrow" }, el("span", { className: "name" }, `${l.side} leg`), el("code", {}, `${l.cross_section} · ${l.length} m`), rm));
    });
    $("jcontrol").value = jv.spec.control;
    $("jcross").checked = !!jv.spec.crosswalks;
    $("jcancel").hidden = !jv.proposed;
  }

  $("road").hidden = !s.road || !!pj || !!cv || !!hv;
  if (s.road && !pj) drawItems($("items"), $("bar"), $("total"), s.road, !s.pending);

  const pr = s.pending && !pj && !pc && !ph ? s.pending : null; // a proposed road
  $("proposal").hidden = !pr;
  fillList("pchanges", pr ? pr.changes : null);
  $("asked").textContent = pr?.description ? `You asked: “${pr.description}”` : "";
  $("asked").hidden = !pr?.description;
  if (pr) {
    drawItems($("pitems"), $("pbar"), $("ptotal"), pr, true);
    $("shorthand").value = pr.canonical;
  } else if (s.road && document.activeElement !== $("shorthand")) $("shorthand").value = s.road.text;

  const scope = $("scope");
  const multi = s.road && !pj && s.road.pieces.total > s.road.pieces.selected;
  scope.hidden = !multi;
  if (multi) $("scope-all").textContent = `All ${s.road.pieces.total} pieces of this road`;

  const wordsFor = t?.kind === "crossing" ? "Describe the intersection, or the change you want (lighted, stop signs, left turn pockets, driveway, ramp, railroad, roundabout …)" : t?.kind === "hub" ? "Describe the intersection, or the change you want (lighted, all-way stop, crosswalks …)" : null;
  const junctionish = t?.kind === "junction" || (t?.kind === "line" && s.mode === "connection");
  $("wordsLabel").textContent = wordsFor ?? (junctionish ? "Describe the junction, or the change you want" : "Describe the road, or the change you want");
  const canAct = !!t && t.kind !== "other" && !s.busy;
  $("propose").disabled = !canAct || !$("words").value.trim();
  $("draw").disabled = !canAct;
  $("draw").textContent = s.busy === "drawing" ? "Drawing…" : s.pending ? "Draw this" : (t?.kind === "crossing" && !s.crossing?.described) || (t?.kind === "hub" && !s.hub?.described) ? "Draw this intersection" : "Redraw";
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
$("jcancel").onclick = () => act("cancel");
$("ccancel").onclick = () => act("cancel");
const cpatch = (patch) => act("crossing", { patch });
$("ctreat").onchange = () => cpatch({ treatment: $("ctreat").value });
$("ccross").onchange = () => cpatch({ crosswalks: $("ccross").checked });
$("cminor").onchange = () => cpatch({ minor: $("cminor").value || null });
$("cover").onchange = () => cpatch({ over: $("cover").value || null });
$("cstub").onchange = () => cpatch({ stub: $("cstub").value });
$("cmain").onchange = () => cpatch({ main: $("cmain").value || null });
$("cbeacon").onchange = () => cpatch({ beacon: $("cbeacon").checked });
$("cgates").onchange = () => cpatch({ gates: $("cgates").checked });
$("cslip").onchange = () => cpatch({ slip: $("cslip").checked });
$("cpocket").onchange = () => cpatch({ pocket: $("cpocket").value });
$("cpocketlen").onchange = () => cpatch({ pocket_length: Number($("cpocketlen").value) || 30 });
$("copen").onchange = () => cpatch({ opening: Number($("copen").value) || null });
$("caccel").onchange = () => cpatch({ accel_length: Number($("caccel").value) || 120 });
const hpatch = (patch) => act("hub", { patch });
$("htreat").onchange = () => hpatch({ treatment: $("htreat").value });
$("hcross").onchange = () => hpatch({ crosswalks: $("hcross").checked });
$("hstub").onchange = () => hpatch({ stub: $("hstub").value });
$("hcancel").onclick = () => act("cancel");
$("cdia").onchange = () => cpatch({ diameter: Number($("cdia").value) || 40 });
$("match").onclick = () => act("match");
for (const r of document.querySelectorAll("input[name=mode]")) r.onchange = () => act("mode", { mode: r.value });
$("jcontrol").onchange = () => act("junction", { patch: { control: $("jcontrol").value } });
$("jcross").onchange = () => act("junction", { patch: { crosswalks: $("jcross").checked } });
$("use").onclick = () => act("text", { text: $("shorthand").value });
$("agent").onchange = () => act("agent", { id: $("agent").value });

const events = new EventSource("/events");
events.onmessage = (m) => show(JSON.parse(m.data));
events.onerror = () => $("dot").classList.remove("on");
