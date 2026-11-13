#!/usr/bin/env node
// Checks library/standards.json and writes docs/STANDARDS.md from it. Zero dependencies.
//   node scripts/standards.mjs check     (exit 1 on a problem)
//   node scripts/standards.mjs md        (rewrite docs/STANDARDS.md)
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const db = JSON.parse(readFileSync(resolve(root, "library/standards.json"), "utf8"));
const FACTOR = { in: 0.0254, ft: 0.3048, mph: 0.44704 };

export function problems(d = db) {
  const bad = [];
  const ids = new Set();
  for (const e of d.entries) {
    const at = `entry ${e.id}`;
    if (ids.has(e.id)) bad.push(`${at}: duplicate id`);
    ids.add(e.id);
    if (!["verified", "verified-secondary", "unverified"].includes(e.status)) bad.push(`${at}: status must be verified, verified-secondary or unverified`);
    if (e.status !== "unverified") {
      if (!d.sources[e.source]) bad.push(`${at}: source "${e.source}" is not in sources`);
      if (!e.section) bad.push(`${at}: a verified entry names its section`);
      if (!e.quote) bad.push(`${at}: a verified entry quotes its source`);
    } else if (e.source && !d.sources[e.source]) bad.push(`${at}: source "${e.source}" is not in sources`);
    if (e.status === "verified" && d.sources[e.source]?.kind !== "federal-standard") bad.push(`${at}: only a federal source can be "verified"; a state manual is verified-secondary`);
    for (const v of e.values ?? []) {
      for (const [us, si] of [["us", "si"], ["min_us", "min_si"], ["max_us", "max_si"]]) {
        if (v[us] === undefined || v[si] === undefined) continue;
        const f = FACTOR[v.unit_us];
        if (!f) bad.push(`${at} (${v.name}): unknown unit ${v.unit_us}`);
        else if (Math.abs(v[us] * f - v[si]) > Math.max(0.0006, Math.abs(v[si]) * 0.002)) bad.push(`${at} (${v.name}): ${v[us]} ${v.unit_us} is not ${v[si]} m`);
      }
    }
    for (const c of e.code ?? []) if (!("conforms" in c) || !c.name || !c.note) bad.push(`${at}: each code entry needs name, conforms (true, false or null) and note`);
  }
  for (const [k, s] of Object.entries(d.sources)) for (const f of ["title", "publisher", "kind", "url", "retrieved", "copyright"]) if (!s[f]) bad.push(`source ${k}: missing ${f}`);
  return bad;
}

const num = (x) => (Number.isInteger(x) ? String(x) : String(+x.toFixed(3)));
function fmt(v) {
  const u = v.unit_us, s = v.unit_si === "m/s" ? "m/s" : "m";
  const one = (us, si) => `${num(us)} ${u}${si !== undefined ? ` (${num(si)} ${s})` : ""}`;
  let out;
  if (v.min_us !== undefined || v.max_us !== undefined) out = `${one(v.min_us, v.min_si)} to ${one(v.max_us, v.max_si)}`;
  else if (v.us !== undefined && u) out = one(v.us, v.si);
  else if (v.us !== undefined) out = num(v.us);
  const extra = v.text ?? v.formula ?? v.ratio;
  if (v.table_us) return `${v.name}: ${Object.entries(v.table_us).map(([k, x]) => `${k} ${x}`).join(", ")}`;
  return `${v.name}: ${[out, extra].filter(Boolean).join(" ")}`;
}

function markdown() {
  const L = [];
  const by = {};
  for (const e of db.entries) (by[e.topic] ??= []).push(e);
  L.push("# Road design standards used by Road Builder", "");
  L.push("Generated from `library/standards.json` by `node scripts/standards.mjs md`. Edit the JSON, not this file.", "");
  L.push(db.note, "");
  L.push("**The NTSB** investigates accidents and recommends changes; it does not publish design dimensions. The federal source for markings, signs and signals is the FHWA MUTCD; for pedestrian facilities the US Access Board; for lane, kerb and turn-lane geometry the AASHTO Green Book (copyrighted, not reproduced here) and state design manuals.", "");
  const count = (s) => db.entries.filter((e) => e.status === s).length;
  L.push(`**${db.entries.length} entries:** ${count("verified")} verified in the primary text, ${count("verified-secondary")} verified in a state manual, ${count("unverified")} unverified.`, "");
  L.push("## Sources", "", "| Key | Source | Kind | Retrieved |", "|---|---|---|---|");
  for (const [k, s] of Object.entries(db.sources)) L.push(`| \`${k}\` | [${s.title}](${s.url}), ${s.publisher} | ${s.kind} | ${s.retrieved} |`);
  L.push("");
  for (const [k, s] of Object.entries(db.sources)) L.push(`- \`${k}\`: ${s.note}${s.file_sha256 ? ` File SHA-256 \`${s.file_sha256.slice(0, 16)}…\`.` : ""} ${s.copyright}.`);
  L.push("");
  for (const [topic, list] of Object.entries(by)) {
    L.push(`## ${topic[0].toUpperCase()}${topic.slice(1)}`, "");
    for (const e of list) {
      L.push(`### ${e.title}`, `\`${e.id}\` · ${e.status} · ${e.source ? `${db.sources[e.source]?.publisher.replace("US DOT ", "")} ${e.source === "mutcd" ? "MUTCD " : ""}${e.section ?? ""}` : "no source yet"} · ${e.kind}`, "");
      for (const v of e.values ?? []) L.push(`- ${fmt(v)}`);
      if (e.quote) L.push("", `> ${e.quote}`);
      for (const c of e.code ?? []) L.push("", `Plug-in: ${c.name}: ${c.conforms === true ? "**conforms**" : c.conforms === false ? "**does not conform**" : "not comparable"}. ${c.note}`);
      L.push("");
    }
  }
  L.push("## Where the plug-in differs from the standard", "", "| Entry | What the plug-in does | What to change |", "|---|---|---|");
  for (const e of db.entries) for (const c of e.code ?? []) if (c.conforms === false) L.push(`| \`${e.id}\` | ${c.name} | ${c.note.replace(/\|/g, "/")} |`);
  L.push("", "## Not retrieved", "");
  for (const b of db.sources_blocked) L.push(`- **${b.what}.** Tried: ${b.tried}. ${b.action}`);
  L.push("", "## Adding an entry", "", "Read the section in the primary text, copy a short verbatim quote, give the section and paragraph, convert units with `unit_us` (in, ft, mph) and let `node scripts/standards.mjs check` verify the metric figure, then regenerate this file. Mark a value `verified` only if it came from a federal source you read; a state manual makes it `verified-secondary`; anything from memory is `unverified` and must not be described to a user as a standard.", "");
  return L.join("\n");
}

if (process.argv[1] === new URL(import.meta.url).pathname) {
  const cmd = process.argv[2];
  if (cmd === "check") {
    const bad = problems();
    for (const b of bad) console.error(b);
    console.log(bad.length ? `${bad.length} problem(s)` : `ok: ${db.entries.length} entries`);
    process.exit(bad.length ? 1 : 0);
  } else if (cmd === "md") {
    writeFileSync(resolve(root, "docs/STANDARDS.md"), markdown());
    console.log("wrote docs/STANDARDS.md");
  } else {
    console.error("usage: standards.mjs check | md");
    process.exit(2);
  }
}
