// One layer per road: "road r1" holds a "road r1 spine" (the line you edit) and a "road r1 drawing" (everything built from it), so the
// red spine can be hidden or locked while the road stays on show. Junctions get the same two parts; an intersection where two roads
// cross gets one layer of drawing (it has no line of its own).
//
// A rebuild only sees objects that are visible and unlocked, so a hidden spine would make its road's drawing vanish. `unlockOurs`
// shows and unlocks our layers for the build and hands back a function that puts them back as the user had them.
import { junctionId, segmentId } from "./doc.mjs";

const OURS = /^(road|junction|crossing) \S+( spine| drawing)?$/;

/** Every layer in the document, flat: {id, name, visible, locked, top (is it a top-level layer), index (position, 0 = top)}. */
async function layerRows(control) {
  const r = await control.call("document.inspect", { depth: 8 });
  const rows = [];
  const walk = (list, top) => (list ?? []).forEach((x, index) => {
    if (x.kind !== "Layer") return;
    rows.push({ id: x.id, name: x.name ?? "", visible: x.visible !== false, locked: !!x.locked, top, index });
    walk(x.children, false);
  });
  walk(r.layers, true);
  return rows;
}

/** Which road, junction or crossing a path belongs to, and whether it is the editable line or part of the drawing. */
export function ownerOf(name = "") {
  const road = segmentId(name);
  if (road) return { layer: `road ${road}`, part: "spine" };
  const junction = junctionId(name);
  if (junction) return { layer: `junction ${junction}`, part: "spine" };
  let m = /^gen\.road\.([^.]+)\./.exec(name);
  if (m) return { layer: `road ${m[1]}`, part: "drawing" };
  m = /^gen\.junction\.([^.]+)\./.exec(name);
  if (m) return { layer: `junction ${m[1]}`, part: "drawing" };
  m = /^gen\.crossing\.([^.]+)\./.exec(name);
  if (m) return { layer: `crossing ${m[1]}`, part: "drawing" };
  return null;
}

/** Shows and unlocks our layers so a rebuild sees everything. Returns an async function that restores what was hidden or locked. */
export async function unlockOurs(control) {
  const changed = (await layerRows(control)).filter((l) => OURS.test(l.name) && (!l.visible || l.locked));
  for (const l of changed) await control.run("layer.setProps", { id: l.id, visible: true, locked: false });
  return async () => {
    for (const l of changed) await control.run("layer.setProps", { id: l.id, visible: l.visible, locked: l.locked });
  };
}

const idOf = (r) => r?.id ?? r?.ids?.[0];

/** Puts every road line, junction line and built shape into its layer (making layers as needed). Leaves other objects alone. */
export async function organise(control) {
  const rows = await layerRows(control);
  const byName = new Map(rows.map((l) => [l.name, l.id]));
  const paths = (await control.call("document.inspect", { kind: "Path", limit: 100000 })).nodes ?? [];

  const want = new Map(); // layer name -> {spine: [], drawing: []}
  for (const p of paths) {
    const o = ownerOf(p.name ?? "");
    if (!o) continue;
    if (!want.has(o.layer)) want.set(o.layer, { spine: [], drawing: [] });
    want.get(o.layer)[o.part].push(p);
  }

  const ensure = async (name, make) => {
    if (!byName.has(name)) byName.set(name, idOf(await make()));
    return byName.get(name);
  };
  for (const [name, parts] of [...want].sort(([a], [b]) => a.localeCompare(b))) {
    const crossing = name.startsWith("crossing ");
    const top = await ensure(name, () => control.run("layer.new", { name, top: true }));
    // The drawing sublayer is made first so the line (made after it) sits above it and stays easy to click.
    const targets = crossing ? { drawing: top } : { drawing: await ensure(`${name} drawing`, () => control.run("layer.newSublayer", { parent: top, name: `${name} drawing` })), spine: await ensure(`${name} spine`, () => control.run("layer.newSublayer", { parent: top, name: `${name} spine` })) };
    for (const part of ["drawing", "spine"]) {
      const target = targets[part];
      const loose = (parts[part] ?? []).filter((p) => p.path?.at(-1) !== target).map((p) => p.id);
      if (target && loose.length) await control.run("layer.move", { ids: loose, target, place: "inside" });
    }
  }

  // Intersections and junctions are drawn over the roads they join: keep their layers above the road layers.
  const now = await layerRows(control);
  const topLayers = now.filter((l) => l.top);
  const firstRoad = topLayers.find((l) => l.name.startsWith("road "));
  if (firstRoad) {
    for (const l of topLayers.filter((x) => /^(junction|crossing) /.test(x.name) && x.index > firstRoad.index)) {
      await control.run("layer.move", { ids: [l.id], target: firstRoad.id, place: "above" });
    }
  }
  return [...want.keys()];
}
