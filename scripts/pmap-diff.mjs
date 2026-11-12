// Compares two pmap road files and says in plain words how they differ. Exits 1 on any difference beyond tolerance.
// Usage: node scripts/pmap-diff.mjs a.pmap b.pmap
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Value of a profile curve (a list of [s, value] points) at route metre s. */
export function curveAt(pts, s) {
  if (s <= pts[0][0]) return pts[0][1];
  const last = pts.at(-1);
  if (s >= last[0]) return last[1];
  let i = 1;
  while (pts[i][0] < s) i++;
  const [a, b] = [pts[i - 1], pts[i]];
  return b[0] === a[0] ? b[1] : a[1] + ((b[1] - a[1]) * (s - a[0])) / (b[0] - a[0]);
}

/** Differences between two parsed pmap files, as sentences (empty list = the same road). */
export function compareFiles(a, b) {
  const problems = [];
  if (a.centerline.length !== b.centerline.length) problems.push(`centerline: ${a.centerline.length} vs ${b.centerline.length} points`);
  else a.centerline.forEach((p, i) => { if (Math.hypot(p[0] - b.centerline[i][0], p[1] - b.centerline[i][1]) > 0.002) problems.push(`centerline point ${i} moved`); });
  for (const k of new Set([...Object.keys(a.profile), ...Object.keys(b.profile)])) {
    if (!a.profile[k] || !b.profile[k]) { problems.push(`profile.${k} missing in one file`); continue; }
    let worst = 0;
    for (let s = 0; s <= a.length; s += 0.5) worst = Math.max(worst, Math.abs(curveAt(a.profile[k], s) - curveAt(b.profile[k], s)));
    if (worst > 0.01) problems.push(`profile.${k} differs by up to ${worst.toFixed(3)} m`);
  }
  for (const f of new Set([...Object.keys(a.flags), ...Object.keys(b.flags)])) {
    const [x, y] = [a.flags[f] ?? [], b.flags[f] ?? []];
    if (x.length !== y.length || x.some((r, i) => Math.abs(r[0] - y[i][0]) > 0.01 || Math.abs(r[1] - y[i][1]) > 0.01)) problems.push(`flag ${f} ranges differ`);
  }
  const byId = new Map(b.junctions.map((j) => [j.id, j]));
  for (const j of a.junctions) {
    const o = byId.get(j.id);
    if (!o) problems.push(`junction ${j.id} (${j.label || j.kind}) is missing`);
    else if (j.kind !== o.kind || j.side !== o.side || j.label !== o.label) problems.push(`junction ${j.id} changed kind, side or street name`);
    else if (Math.abs(j.s - o.s) > 0.01) problems.push(`junction ${j.id} (${j.label || j.kind}) moved from ${j.s} m to ${o.s} m`);
  }
  for (const o of b.junctions) if (!a.junctions.some((j) => j.id === o.id)) problems.push(`junction ${o.id} (${o.label || o.kind}) is new, at ${o.s} m`);
  if (a.arrows.length !== b.arrows.length) problems.push(`arrows: ${a.arrows.length} vs ${b.arrows.length}`);
  else a.arrows.forEach((x, i) => { const y = b.arrows[i]; if (x.kind !== y.kind || x.south !== y.south || Math.abs(x.s - y.s) > 0.01 || Math.abs(x.off - y.off) > 0.01) problems.push(`arrow ${i} differs`); });
  return problems;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [fa, fb] = process.argv.slice(2);
  if (!fa || !fb) {
    console.error("usage: pmap-diff.mjs a.pmap b.pmap");
    process.exit(2);
  }
  const load = (f) => JSON.parse(readFileSync(resolve(f), "utf8"));
  const problems = compareFiles(load(fa), load(fb));
  if (problems.length) {
    console.log(`DIFFERENT (${problems.length}):\n- ${problems.join("\n- ")}`);
    process.exit(1);
  }
  console.log("OK: the two pmap files describe the same road");
}
