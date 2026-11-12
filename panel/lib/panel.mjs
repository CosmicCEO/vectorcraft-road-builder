// The panel's brain: follows the selection in VectorCraft, shows the selected road's description, turns words into a proposed
// change with an agent, and on Draw applies it (Describe Road, then Build Road) without losing the user's selection.
import { EventEmitter } from "node:events";
import { editCrossSection, parseCrossSection, parseCrossing, parseJunction } from "./cli.mjs";
import { PLUGIN_ID, SET_CROSSING, SET_CROSS_SECTION, SET_JUNCTION, allPaths, classify, inspectState, junctionId, nodes, readSpec, segmentId } from "./doc.mjs";
import { organise, unlockOurs } from "./layers.mjs";
import { propose as proposeWith, proposeCrossing, proposeJunction } from "./providers.mjs";

const DEFAULT_TEXT = "lane back | lane fwd";
const DEFAULT_CROSSING = { treatment: "signal", crosswalks: true, radius: 6, minor: null, over: null, diameter: 40, stub: "keep" };
const NOT_BUILT = "A cloverleaf is not built yet; an overpass is the closest for now.";

export class Panel extends EventEmitter {
  constructor({ control, providers }) {
    super();
    this.control = control;
    this.providers = providers;
    this.provider = providers.default;
    this.state = { connected: false, error: null, doc: null, selection: [], target: null, road: null, junction: null, crossing: null, mode: "road", pending: null, busy: null, message: null };
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
      return this.#set({ connected: !noDoc && this.control.connected, error: noDoc ? "Open or create a document in VectorCraft." : e.message, doc: null, selection: [], target: null, road: null, junction: null, crossing: null, pending: null });
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
        else spines = ((this.spec = await readSpec(this.control)).crossings?.[n.segment]?.roads ?? []).map((r) => road(r));
        return spines.filter((p) => p && !seen.has(p.id)).map((p) => (seen.add(p.id), { id: p.id, name: p.name, kind: "Path", role: junctionId(p.name) ? "junction" : "road", segment: segmentId(p.name) ?? junctionId(p.name), viaShape: true }));
      }))).flat();
    }
    const segments = [...new Set(sel.filter((n) => n.role === "road").map((n) => n.segment))];
    let target = null;
    if (segments.length === 1 && sel.every((n) => n.role === "road")) target = { kind: "road", segment: segments[0], ids: sel.map((n) => n.id) };
    else if (segments.length === 2 && sel.every((n) => n.role === "road")) target = { kind: "crossing", roads: [...segments].sort(), ids: sel.map((n) => n.id) };
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
    // A proposal belongs to what was selected when it was made.
    const keep = this.state.pending && !selectionChanged;
    this.#set({ connected: true, error: null, doc: { title: s.title, revision: s.revision }, selection: sel, target, road, junction, crossing, pending: keep ? this.state.pending : null, message: keep ? this.state.message : null });
  }

  /** Words to a proposed cross-section (not applied until Draw). */
  propose(description, providerId) {
    return this.#serial(async () => {
      const t = this.state.target;
      if (!t || t.kind === "other") return this.#set({ message: "Select one line (or one road) first." });
      const provider = this.providers.providers[providerId ?? this.provider];
      if (!provider) return this.#set({ message: `Unknown agent "${providerId}".` });
      this.#set({ busy: "thinking", message: null });
      try {
        if (t.kind === "crossing") {
          const c = this.state.crossing;
          const r = await proposeCrossing({ description, current: c?.described ? c.spec : null, roads: c.roads, provider });
          return this.#set(r.ok ? { pending: { kind: "crossing", crossing: r.crossing, summary: r.summary, source: "agent", agent: providerId ?? this.provider, description }, message: null } : { message: r.error });
        }
        const asJunction = t.kind === "junction" || (t.kind === "line" && this.state.mode === "connection");
        if (asJunction) {
          const j = this.state.junction;
          const current = j?.described ? j.spec : null;
          const r = await proposeJunction({ description, current, provider });
          return this.#set(r.ok ? { pending: { kind: "junction", junction: r.junction, summary: r.summary, source: "agent", agent: providerId ?? this.provider, description }, message: null } : { message: r.error });
        }
        const current = this.state.road?.described ? this.state.road.text : null;
        const r = await proposeWith({ description, current, provider });
        this.#set(r.ok ? { pending: { ...r, source: "agent", agent: providerId ?? this.provider, description }, message: r.notes?.length ? `The agent read: ${r.notes.join("; ")}` : null } : { message: r.error });
      } catch (e) {
        this.#set({ message: e.message });
      } finally {
        this.#set({ busy: null });
      }
    });
  }

  /** Edits the proposed (or current) cross-section: ["delete", i] | ["move", from, to] | ["width", i, m] | ["insert", i, "text"]. */
  edit(op) {
    return this.#serial(async () => {
      const base = this.state.pending?.canonical ?? this.state.road?.text;
      if (!base) return this.#set({ message: "Select a road or a line first." });
      const r = await editCrossSection(base, op);
      this.#set(r.ok ? { pending: { ...r, source: "edit" }, message: null } : { message: r.error });
    });
  }

  /** The user typed the shorthand themselves. */
  setText(text) {
    return this.#serial(async () => {
      const r = await parseCrossSection(text);
      this.#set(r.ok ? { pending: { ...r, source: "typed" }, message: r.notes?.length ? `Read as: ${r.notes.join("; ")}` : null } : { message: r.error });
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
    if (mode !== "road" && mode !== "connection") return false;
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
      this.#set(r.ok ? { pending: { kind: "junction", junction: r.junction, summary: r.summary, source: "edit" }, message: null } : { message: r.error });
    });
  }

  /** Quick choices for the selected crossing: {treatment, crosswalks, radius, minor, over, diameter, stub}. Starts a proposal from the current (or default) description. */
  editCrossing(patch) {
    return this.#serial(async () => {
      const base = this.state.pending?.crossing ?? this.state.crossing?.spec;
      if (!base || this.state.target?.kind !== "crossing") return this.#set({ message: "Select the two crossing roads first." });
      const next = { ...base, roads: this.state.crossing.roads };
      for (const k of ["treatment", "crosswalks", "radius", "minor", "over", "diameter", "stub"]) if (patch[k] !== undefined) next[k] = patch[k];
      const r = await parseCrossing(JSON.stringify(next));
      this.#set(r.ok ? { pending: { kind: "crossing", crossing: r.crossing, summary: r.summary, source: "edit" }, message: r.crossing.treatment === "cloverleaf" ? NOT_BUILT : null } : { message: r.error });
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
        if (t.kind === "crossing") {
          const spec = pending?.kind === "crossing" ? pending.crossing : this.state.crossing.spec;
          if (spec.treatment === "cloverleaf") throw new Error(NOT_BUILT);
          await this.control.run("select.set", { ids: t.ids });
          await this.control.run(SET_CROSSING, { params: { description: JSON.stringify(spec) } });
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
        this.#set({ pending: null, message: pending || t.kind === "crossing" ? "Drawn." : "Rebuilt." });
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
    return { ...this.state, agents: { default: this.provider, list: Object.entries(this.providers.providers).map(([id, p]) => ({ id, label: p.label ?? id, kind: p.kind })) } };
  }

  setAgent(id) {
    if (!this.providers.providers[id]) return false;
    this.provider = id;
    this.emit("change", this.snapshot());
    return true;
  }
}
