// The panel's brain: follows the selection in VectorCraft, shows the selected road's description, turns words into a proposed
// change with an agent, and on Draw applies it (Describe Road, then Build Road) without losing the user's selection.
import { EventEmitter } from "node:events";
import { diffCrossSection, editCrossSection, parseCrossSection, parseCrossing, parseHub, parseJunction, parseTransition } from "./cli.mjs";
import { describeChanges } from "./changes.mjs";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { PLUGIN_ID, SET_CROSSING, SET_CROSS_SECTION, SET_HUB, SET_TRANSITION, SET_JUNCTION, allPaths, classify, inspectState, junctionId, nodes, readSpec, segmentId } from "./doc.mjs";
import { organise, unlockOurs } from "./layers.mjs";
import { propose as proposeWith, proposeCrossing, proposeHub, proposeJunction, proposeTransition } from "./providers.mjs";

const DEFAULT_TEXT = "lane back | lane fwd";
const DEFAULT_CROSSING = { treatment: "signal", crosswalks: true, minor: null, over: null, diameter: 40, stub: "keep" };
const DEFAULT_HUB = { treatment: "signal", crosswalks: true, minor: null, stub: "keep" };
const CROSSING_KEYS = ["treatment", "crosswalks", "radius", "minor", "over", "main", "diameter", "stub", "pocket", "pocket_length", "pocket_taper", "slip", "slip_radius", "opening", "gore", "gore_length", "accel_length", "accel_taper", "beacon", "gates"];
const DEFAULT_TRANSITION = { change: "add", side: "right", direction: "fwd", count: 1 };
const HUB_KEYS = ["treatment", "crosswalks", "radius", "minor", "stub"];

export class Panel extends EventEmitter {
  constructor({ control, providers }) {
    super();
    this.control = control;
    this.providers = providers;
    this.provider = providers.default;
    this.state = { connected: false, error: null, doc: null, selection: [], notes: [], target: null, road: null, junction: null, crossing: null, hub: null, mode: "road", pending: null, busy: null, message: null };
    this.last = { revision: null, selection: "" };
    this.spec = null;
    this.queue = Promise.resolve();
    this.timer = null;
  }

  /** Runs `fn` after everything queued before it (the poll and user actions must not overlap). */
  #serial(fn) {
    const run = this.queue.then(fn, fn);
    this.queue = run.catch(() => {});
    return run;
  }

  start(intervalMs = 400) {
    this.timer = setInterval(() => this.tick(), intervalMs);
    return this.tick();
  }

  stop() {
    clearInterval(this.timer);
  }

  tick(force = false) {
    if (this.state.busy) return Promise.resolve();
    return this.#serial(() => this.#refresh(force));
  }

  #set(patch) {
    const before = JSON.stringify(this.snapshot());
    Object.assign(this.state, patch);
    if (JSON.stringify(this.snapshot()) !== before) this.emit("change", this.snapshot());
  }

  async #refresh(force) {
    let s;
    try {
      s = await inspectState(this.control);
    } catch (e) {
      const noDoc = /no (active )?document/i.test(e.message);
      this.last = { revision: null, selection: "" };
      return this.#set({ connected: !noDoc && this.control.connected, error: noDoc ? "Open or create a document in VectorCraft." : e.message, doc: null, selection: [], target: null, road: null, junction: null, crossing: null, hub: null, pending: null });
    }
    const key = s.selection.join(",");
    const changed = force || key !== this.last.selection || s.revision !== this.last.revision;
    if (!changed) return this.#set({ connected: true, error: null });
    const selectionChanged = key !== this.last.selection;
    this.last = { revision: s.revision, selection: key };

    let sel = (await nodes(this.control, s.selection)).map((n) => ({ id: n.id, name: n.name ?? "", kind: n.kind, anchors: n.anchors, ...classify(n) }));
    // Clicking anywhere on a built road selects one of its generated shapes. For the panel that means "this road": swap each shape for the
    // line it was built from (the shape's piece number is the Nth line of that road, in document order).
    if (sel.some((n) => /-shape$/.test(n.role))) {
      const paths = await allPaths(this.control);
      const seen = new Set();
      const road = (id, piece = 1) => paths.filter((p) => segmentId(p.name) === id)[piece - 1];
      sel = (await Promise.all(sel.map(async (n) => {
        if (!/-shape$/.test(n.role)) return [n];
        let spines = [];
        if (n.role === "road-shape") spines = [road(n.segment, n.piece)];
        else if (n.role === "junction-shape") spines = [paths.find((p) => junctionId(p.name) === n.segment)];
        else if (n.role === "crossing-shape") spines = ((this.spec = await readSpec(this.control)).crossings?.[n.segment]?.roads ?? []).map((r) => road(r));
        else spines = ((this.spec = await readSpec(this.control)).hubs?.[n.segment]?.roads ?? []).map((r) => road(r));
        return spines.filter((p) => p && !seen.has(p.id)).map((p) => (seen.add(p.id), { id: p.id, name: p.name, kind: "Path", role: junctionId(p.name) ? "junction" : "road", segment: segmentId(p.name) ?? junctionId(p.name), viaShape: true }));
      }))).flat();
    }
    const segments = [...new Set(sel.filter((n) => n.role === "road").map((n) => n.segment))];
    let target = null;
    if (segments.length === 1 && sel.every((n) => n.role === "road")) target = { kind: "road", segment: segments[0], ids: sel.map((n) => n.id) };
    else if (segments.length === 2 && sel.every((n) => n.role === "road")) target = { kind: "crossing", roads: [...segments].sort(), ids: sel.map((n) => n.id) };
    else if (segments.length >= 3 && sel.every((n) => n.role === "road")) target = { kind: "hub", roads: [...segments].sort(), ids: sel.map((n) => n.id) };
    else if (sel.every((n) => n.role === "junction") && sel.length && new Set(sel.map((n) => n.segment)).size === 1) target = { kind: "junction", segment: sel[0].segment, ids: sel.map((n) => n.id) };
    else if (sel.length === 1 && sel[0].role === "line") target = { kind: "line", ids: [sel[0].id] };
    else if (sel.length) target = { kind: "other", ids: sel.map((n) => n.id) };

    let road = null;
    if (target?.kind === "road") {
      this.spec = await readSpec(this.control);
      const text = this.spec.segments?.[target.segment]?.cross_section ?? null;
      const parsed = await parseCrossSection(text ?? DEFAULT_TEXT);
      const total = (await allPaths(this.control)).filter((p) => p.name === `road.${target.segment}` || (target.segment === "main" && p.name === "road")).length;
      road = { segment: target.segment, described: text !== null, text: text ?? DEFAULT_TEXT, items: parsed.items ?? [], total_width: parsed.total_width, anchor: this.spec.segments?.[target.segment]?.anchor ?? "auto", pieces: { selected: target.ids.length, total } };
      // Which line of the road is selected (for a lane change: transitions belong to one line), and the lane changes stored for it.
      const lines = (await allPaths(this.control)).filter((p) => segmentId(p.name) === target.segment);
      road.piece = target.ids.length === 1 ? lines.findIndex((p) => p.id === target.ids[0]) + 1 || null : null;
      road.transitions = [];
      for (const e of this.spec.transitions?.[target.segment] ?? []) {
        if (road.piece && (e.piece ?? 1) !== road.piece) continue;
        const r = await parseTransition(JSON.stringify(e), road.text);
        road.transitions.push({ spec: e, summary: r.ok ? r.summary : "", after: r.ok ? r.after : "" });
      }
    }
    let junction = null;
    if (target?.kind === "junction") {
      this.spec = await readSpec(this.control);
      const stored = this.spec.junctions?.[target.segment] ?? null;
      const parsed = stored ? await parseJunction(JSON.stringify(stored)) : null;
      junction = { id: target.segment, described: !!stored, spec: parsed?.ok ? parsed.junction : { legs: [], control: "none", crosswalks: false, radius: 6, kind: "match" }, summary: parsed?.ok ? parsed.summary : "road carried across the gap (not described yet)" };
    }
    let crossing = null;
    if (target?.kind === "crossing") {
      this.spec = await readSpec(this.control);
      const [a, b] = target.roads;
      const stored = Object.entries(this.spec.crossings ?? {}).find(([, c]) => c.roads?.length === 2 && c.roads.includes(a) && c.roads.includes(b));
      const parsed = await parseCrossing(JSON.stringify({ ...(stored?.[1] ?? DEFAULT_CROSSING), roads: [a, b] }));
      crossing = { id: stored?.[0] ?? null, roads: [a, b], described: !!stored, spec: parsed.ok ? parsed.crossing : { ...DEFAULT_CROSSING, roads: [a, b] }, summary: parsed.ok ? parsed.summary : "" };
    }
    let hub = null;
    if (target?.kind === "hub") {
      this.spec = await readSpec(this.control);
      const same = (h) => h.roads?.length === target.roads.length && target.roads.every((r) => h.roads.includes(r));
      const stored = Object.entries(this.spec.hubs ?? {}).find(([, h]) => same(h));
      const parsed = await parseHub(JSON.stringify({ ...(stored?.[1] ?? DEFAULT_HUB), roads: target.roads }));
      hub = { id: stored?.[0] ?? null, roads: target.roads, described: !!stored, spec: parsed.ok ? parsed.hub : { ...DEFAULT_HUB, roads: target.roads }, summary: parsed.ok ? parsed.summary : "" };
    }
    // A proposal belongs to what was selected when it was made.
    const keep = this.state.pending && !selectionChanged;
    this.#set({ connected: true, error: null, doc: { title: s.title, revision: s.revision }, notes: this.spec?.notes ?? [], selection: sel, target, road, junction, crossing, hub, pending: keep ? this.state.pending : null, message: keep ? this.state.message : null });
  }

  /** Words to a proposed cross-section (not applied until Draw). */
  propose(description, providerId) {
    return this.#serial(async () => {
      const t = this.state.target;
      if (!t || t.kind === "other") return this.#set({ message: "Select one line (or one road) first." });
      const first = providerId ?? this.provider;
      if (!this.providers.providers[first]) return this.#set({ message: `Unknown agent "${providerId}".` });
      // The chosen agent first; if it cannot give a valid answer, the one named in its `fallback`.
      const order = [first];
      const fb = this.providers.providers[first].fallback;
      if (fb && fb !== first && this.providers.providers[fb] && this.#available(this.providers.providers[fb])) order.push(fb);
      this.#set({ busy: "thinking", message: null });
      try {
        let failure = null;
        for (const [n, id] of order.entries()) {
          const r = await this.#ask(t, description, this.providers.providers[id], id);
          if (r.pending) {
            const label = (x) => this.providers.providers[x].label ?? x;
            const said = r.message ?? (n > 0 ? null : null);
            return this.#set({ pending: r.pending, message: n > 0 ? `${label(first)} could not give a valid answer, so ${label(id)} answered.${said ? ` ${said}` : ""}` : said ?? null });
          }
          failure = r.message;
        }
        this.#set({ message: failure });
      } catch (e) {
        this.#set({ message: e.message });
      } finally {
        this.#set({ busy: null });
      }
    });
  }

  /** One agent's attempt for the current target: {pending, message?} when it gave a valid proposal, else {message}. */
  async #ask(t, description, provider, id) {
    const base = { source: "agent", agent: id, description };
    if (t.kind === "hub") {
      const h = this.state.hub;
      const r = await proposeHub({ description, current: h?.described ? h.spec : null, roads: h.roads, provider });
      return r.ok ? { pending: { kind: "hub", hub: r.hub, summary: r.summary, changes: describeChanges(h?.described ? h.spec : null, r.hub), ...base } } : { message: r.error };
    }
    if (t.kind === "crossing") {
      const c = this.state.crossing;
      const r = await proposeCrossing({ description, current: c?.described ? c.spec : null, roads: c.roads, provider });
      return r.ok ? { pending: { kind: "crossing", crossing: r.crossing, summary: r.summary, changes: describeChanges(c?.described ? c.spec : null, r.crossing), ...base } } : { message: r.error };
    }
    if (t.kind === "junction" || (t.kind === "line" && this.state.mode === "connection")) {
      const j = this.state.junction;
      const current = j?.described ? j.spec : null;
      const r = await proposeJunction({ description, current, provider });
      return r.ok ? { pending: { kind: "junction", junction: r.junction, summary: r.summary, changes: describeChanges(current, r.junction, "junction"), ...base } } : { message: r.error };
    }
    if (t.kind === "road" && this.state.mode === "transition") {
      const road = this.state.road;
      const draft = this.state.pending?.kind === "transition" ? this.state.pending.transition : null;
      const r = await proposeTransition({ description, current: draft, crossSection: road.text, provider });
      return r.ok ? { pending: { kind: "transition", transition: { ...r.transition, piece: road.piece ?? 1 }, summary: r.summary, after: r.after, changes: [r.summary, `after it the road is: ${r.after}`], ...base } } : { message: r.error };
    }
    const current = this.state.road?.described ? this.state.road.text : null;
    const r = await proposeWith({ description, current, provider });
    if (!r.ok) return { message: r.error };
    return { pending: { ...r, changes: await this.#roadChanges(current, r), ...base }, message: r.notes?.length ? `The agent read: ${r.notes.join("; ")}` : null };
  }

  /** Plain words for what a proposed cross-section changes compared with the current one (or what it is, for a new road). */
  async #roadChanges(current, proposed) {
    if (!current) return [`a new road: ${proposed.items?.length ?? 0} elements, ${proposed.total_width} m wide`];
    const d = await diffCrossSection(current, proposed.canonical);
    return d.ok ? (d.changes.length ? d.changes : ["no change"]) : [];
  }

  /** Is an agent usable here? An optional command-line agent is hidden when its program is not installed. */
  #available(p) {
    if (!p.optional || p.kind !== "cli" || !p.command) return true;
    return existsSync(resolve(import.meta.dirname, "../..", p.command)) || existsSync(p.command);
  }

  /** Edits the proposed (or current) cross-section: ["delete", i] | ["move", from, to] | ["width", i, m] | ["insert", i, "text"]. */
  edit(op) {
    return this.#serial(async () => {
      const base = this.state.pending?.canonical ?? this.state.road?.text;
      if (!base) return this.#set({ message: "Select a road or a line first." });
      const r = await editCrossSection(base, op);
      this.#set(r.ok ? { pending: { ...r, source: "edit", changes: await this.#roadChanges(this.state.road?.described ? this.state.road.text : null, r) }, message: null } : { message: r.error });
    });
  }

  /** The user typed the shorthand themselves. */
  setText(text) {
    return this.#serial(async () => {
      const r = await parseCrossSection(text);
      this.#set(r.ok ? { pending: { ...r, source: "typed", changes: await this.#roadChanges(this.state.road?.described ? this.state.road.text : null, r) }, message: r.notes?.length ? `Read as: ${r.notes.join("; ")}` : null } : { message: r.error });
    });
  }

  /** Puts the road's (or junction's) own line in VectorCraft's selection, ready to move, reshape or delete. */
  selectLine() {
    return this.#serial(async () => {
      const t = this.state.target;
      if (!t || (t.kind !== "road" && t.kind !== "junction")) return this.#set({ message: "Select a road or a junction first." });
      await this.control.run("select.set", { ids: t.ids });
      await this.#refresh(true);
      this.#set({ message: "The line is selected in VectorCraft: move or reshape its points to change the road." });
    });
  }

  /** What a plain line is: a road, or a connection between two roads. */
  setMode(mode) {
    if (mode !== "road" && mode !== "connection" && mode !== "transition") return false;
    this.#set({ mode, pending: null, message: null });
    return true;
  }

  /** "Connect and match": the line just carries the road across the gap, no side roads. */
  match() {
    return this.#serial(async () => {
      const r = await parseJunction(JSON.stringify({ legs: [], control: "none", crosswalks: false }));
      this.#set({ pending: { kind: "junction", junction: r.junction, summary: r.summary, source: "match" }, message: null });
    });
  }

  /** Adjusts the proposed (or current) junction: {control, crosswalks, radius} or {removeLeg: i}. */
  editJunction(patch) {
    return this.#serial(async () => {
      const base = this.state.pending?.junction ?? this.state.junction?.spec;
      if (!base) return this.#set({ message: "Select a junction or a connecting line first." });
      const next = { ...base, legs: [...(base.legs ?? [])] };
      for (const k of ["control", "crosswalks", "radius"]) if (patch[k] !== undefined) next[k] = patch[k];
      if (Number.isInteger(patch.removeLeg)) next.legs.splice(patch.removeLeg, 1);
      const r = await parseJunction(JSON.stringify(next));
      this.#set(r.ok ? { pending: { kind: "junction", junction: r.junction, summary: r.summary, source: "edit", changes: describeChanges(this.state.junction?.described ? this.state.junction.spec : null, r.junction, "junction") }, message: null } : { message: r.error });
    });
  }

  /** Quick choices for the selected crossing: {treatment, crosswalks, pocket, slip, …: any field of an intersection description}. Starts a proposal from the current (or default) description. */
  editCrossing(patch) {
    return this.#serial(async () => {
      const base = this.state.pending?.crossing ?? this.state.crossing?.spec;
      if (!base || this.state.target?.kind !== "crossing") return this.#set({ message: "Select the two crossing roads first." });
      const next = { ...base, roads: this.state.crossing.roads };
      for (const k of CROSSING_KEYS) if (patch[k] !== undefined) next[k] = patch[k];
      const r = await parseCrossing(JSON.stringify(next));
      const stored = this.state.crossing.described ? this.state.crossing.spec : null;
      this.#set(r.ok ? { pending: { kind: "crossing", crossing: r.crossing, summary: r.summary, source: "edit", changes: describeChanges(stored, r.crossing) }, message: null } : { message: r.error });
    });
  }

  /** Quick choices for a lane change on the selected line: {change, side, direction, count, at, length, speed}. Starts from the draft (or a default). */
  editTransition(patch) {
    return this.#serial(async () => {
      const road = this.state.road;
      if (this.state.target?.kind !== "road" || !road?.piece) return this.#set({ message: "Select one line of one road first." });
      const base = this.state.pending?.kind === "transition" ? this.state.pending.transition : DEFAULT_TRANSITION;
      const next = { ...base, ...patch, piece: road.piece };
      for (const k of ["at", "length"]) if (next[k] === "" || Number.isNaN(next[k])) next[k] = null;
      const r = await parseTransition(JSON.stringify(next), road.text);
      this.#set(r.ok ? { pending: { kind: "transition", transition: r.transition, summary: r.summary, after: r.after, changes: [r.summary, `after it the road is: ${r.after}`], source: "edit" }, message: null } : { message: r.error });
    });
  }

  /** Takes away the `index`th lane change stored for the selected line. */
  removeTransition(index) {
    return this.#serial(async () => {
      const t = this.state.target;
      const e = this.state.road?.transitions?.[index];
      if (t?.kind !== "road" || !e) return this.#set({ message: "That lane change is not there any more." });
      this.#set({ busy: "drawing", message: null });
      try {
        await this.control.run("select.set", { ids: t.ids });
        await this.control.run(SET_TRANSITION, { params: { description: JSON.stringify({ ...e.spec, remove: true }) } });
        await this.#build(t.ids);
        this.spec = await readSpec(this.control);
        this.#set({ pending: null, notes: this.spec.notes ?? [], message: "Lane change removed." });
      } catch (err) {
        this.#set({ message: `Could not remove it: ${err.message}` });
      } finally {
        this.#set({ busy: null });
        await this.#refresh(true);
      }
    });
  }

  /** Quick choices for three or more selected roads: {treatment, crosswalks, radius, minor, stub}. */
  editHub(patch) {
    return this.#serial(async () => {
      const base = this.state.pending?.hub ?? this.state.hub?.spec;
      if (!base || this.state.target?.kind !== "hub") return this.#set({ message: "Select the roads that meet first." });
      const next = { ...base, roads: this.state.hub.roads };
      for (const k of HUB_KEYS) if (patch[k] !== undefined) next[k] = patch[k];
      const r = await parseHub(JSON.stringify(next));
      const stored = this.state.hub.described ? this.state.hub.spec : null;
      this.#set(r.ok ? { pending: { kind: "hub", hub: r.hub, summary: r.summary, source: "edit", changes: describeChanges(stored, r.hub) }, message: null } : { message: r.error });
    });
  }

  cancel() {
    return this.#serial(async () => this.#set({ pending: null, message: null }));
  }

  /** Draw: apply the proposal (if any), then rebuild every road, and give the user their selection back. */
  draw({ scope } = {}) {
    return this.#serial(async () => {
      const t = this.state.target;
      const pending = this.state.pending;
      if (!t || t.kind === "other") return this.#set({ message: "Select one line (or one road) first." });
      this.#set({ busy: "drawing", message: null });
      try {
        if (t.kind === "hub") {
          const spec = pending?.kind === "hub" ? pending.hub : this.state.hub.spec;
          await this.control.run("select.set", { ids: t.ids });
          await this.control.run(SET_HUB, { params: { description: JSON.stringify(spec) } });
        } else if (t.kind === "crossing") {
          const spec = pending?.kind === "crossing" ? pending.crossing : this.state.crossing.spec;
          await this.control.run("select.set", { ids: t.ids });
          await this.control.run(SET_CROSSING, { params: { description: JSON.stringify(spec) } });
        } else if (pending?.kind === "transition") {
          await this.control.run("select.set", { ids: t.ids });
          await this.control.run(SET_TRANSITION, { params: { description: JSON.stringify(pending.transition) } });
        } else if (pending?.kind === "junction") {
          await this.control.run("select.set", { ids: t.ids });
          await this.control.run(SET_JUNCTION, { params: { description: JSON.stringify(pending.junction) } });
        } else if (pending) {
          await this.control.run("select.set", { ids: t.ids });
          const params = { cross_section: pending.canonical };
          if (scope === "selected") params.scope = "selected";
          await this.control.run(SET_CROSS_SECTION, { params });
        }
        await this.#build(t.ids);
        this.spec = await readSpec(this.control);
        this.#set({ pending: null, notes: this.spec.notes ?? [], message: pending || t.kind === "crossing" || t.kind === "hub" ? "Drawn." : "Rebuilt." });
      } catch (e) {
        this.#set({ message: `Could not draw: ${e.message}` });
      } finally {
        this.#set({ busy: null });
        await this.#refresh(true);
      }
    });
  }

  /** Rebuilds every road. Our layers are shown and unlocked for the build (a hidden spine would make its road vanish), the built shapes are
   *  sorted back into each road's layers, and the user's hiding and locking is put back. */
  async #build(restoreIds) {
    const restore = await unlockOurs(this.control);
    try {
      await this.control.run("select.all");
      await this.control.run("plugin.run", { id: PLUGIN_ID });
      await organise(this.control);
    } finally {
      await restore();
    }
    await this.control.run("select.set", { ids: restoreIds });
  }

  snapshot() {
    return { ...this.state, agents: { default: this.provider, list: Object.entries(this.providers.providers).filter(([, p]) => this.#available(p)).map(([id, p]) => ({ id, label: p.label ?? id, kind: p.kind })) } };
  }

  setAgent(id) {
    if (!this.providers.providers[id]) return false;
    this.provider = id;
    this.emit("change", this.snapshot());
    return true;
  }
}
