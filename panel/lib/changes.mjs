// Plain-words differences between a stored description and a proposed one, for the "What changes" list on each card.
const LABEL = {
  treatment: "type", crosswalks: "crosswalks", radius: "kerb radius", minor: "road that stops", over: "road on top", main: "road that stays", diameter: "width",
  stub: "short stub", stub_length: "stub length", pocket: "turn pockets", pocket_length: "pocket length", pocket_taper: "pocket taper", slip: "slip lanes",
  slip_radius: "slip lane radius", opening: "opening", accel_length: "speed-change lane", accel_taper: "speed-change taper", beacon: "beacons", gates: "gates",
  control: "control", length: "length", side: "side", cross_section: "cross-section",
};
const SKIP = new Set(["roads", "kind"]);

const show = (v) => (v === null || v === undefined ? "automatic" : v === true ? "on" : v === false ? "off" : typeof v === "object" ? JSON.stringify(v) : String(v));

/** Sentences for what changes from `before` to `after` (objects). `before` null means there is nothing stored yet. */
export function describeChanges(before, after, what = "intersection") {
  if (!before) return [`a new ${what}`];
  const out = [];
  for (const key of [...new Set([...Object.keys(before), ...Object.keys(after)])]) {
    if (SKIP.has(key)) continue;
    if (key === "legs") {
      const [a, b] = [before.legs ?? [], after.legs ?? []];
      if (JSON.stringify(a) !== JSON.stringify(b)) out.push(`legs: ${a.length} → ${b.length}${b.length ? ` (${b.map((l) => l.side).join(", ")})` : ""}`);
      continue;
    }
    const [x, y] = [before[key], after[key]];
    if (JSON.stringify(x ?? null) !== JSON.stringify(y ?? null)) out.push(`${LABEL[key] ?? key}: ${show(x)} → ${show(y)}`);
  }
  return out.length ? out : ["no change"];
}
