// The panel's brain: follows the selection in VectorCraft, shows the selected road's description, turns words into a proposed
// change with an agent, and on Draw applies it (Describe Road, then Build Road) without losing the user's selection.
import { EventEmitter } from "node:events";
import { editCrossSection, parseCrossSection } from "./cli.mjs";
import { PLUGIN_ID, SET_CROSS_SECTION, allPaths, classify, inspectState, nodes, readSpec } from "./doc.mjs";
import { propose as proposeWith } from "./providers.mjs";

const DEFAULT_TEXT = "lane fwd | lane back";

export class Panel extends EventEmitter {
  constructor({ control, providers }) {
    super();
    this.control = control;
    this.providers = providers;
    this.provider = providers.default;
    this.state = { connected: false, error: null, doc: null, selection: [], target: null, road: null, pending: null, busy: null, message: null };
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
      return this.#set({ connected: !noDoc && this.control.connected, error: noDoc ? "Open or create a document in VectorCraft." : e.message, doc: null, selection: [], target: null, road: null, pending: null });
    }
    const key = s.selection.join(",");
    const changed = force || key !== this.last.selection || s.revision !== this.last.revision;
    if (!changed) return this.#set({ connected: true, error: null });
    const selectionChanged = key !== this.last.selection;
    this.last = { revision: s.revision, selection: key };

    const sel = (await nodes(this.control, s.selection)).map((n) => ({ id: n.id, name: n.name ?? "", kind: n.kind, anchors: n.anchors, ...classify(n) }));
    const segments = [...new Set(sel.filter((n) => n.role === "road").map((n) => n.segment))];
    let target = null;
    if (segments.length === 1 && sel.every((n) => n.role === "road")) target = { kind: "road", segment: segments[0], ids: sel.map((n) => n.id) };
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
    // A proposal belongs to what was selected when it was made.
    const keep = this.state.pending && !selectionChanged;
    this.#set({ connected: true, error: null, doc: { title: s.title, revision: s.revision }, selection: sel, target, road, pending: keep ? this.state.pending : null, message: keep ? this.state.message : null });
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
        if (pending) {
          await this.control.run("select.set", { ids: t.ids });
          const params = { cross_section: pending.canonical };
          if (scope === "selected") params.scope = "selected";
          await this.control.run(SET_CROSS_SECTION, { params });
        }
        await this.#build(t.ids);
        this.#set({ pending: null, message: pending ? "Drawn." : "Rebuilt." });
      } catch (e) {
        this.#set({ message: `Could not draw: ${e.message}` });
      } finally {
        this.#set({ busy: null });
        await this.#refresh(true);
      }
    });
  }

  async #build(restoreIds) {
    await this.control.run("select.all");
    await this.control.run("plugin.run", { id: PLUGIN_ID });
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
