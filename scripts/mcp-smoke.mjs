// Talks to `vectorcraft-cli mcp` over stdio exactly as an MCP client (Claude Code) does, and drives the Road Builder through it.
// Usage: node scripts/mcp-smoke.mjs <path-to-vectorcraft-cli> <pmap-to-open> [--demo out.pmap | tool-name tool-json-args]
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";

const [cli, pmap, ...rest] = process.argv.slice(2);
if (!cli || !pmap) {
  console.error("usage: mcp-smoke.mjs <vectorcraft-cli> <file.pmap> [tool args-json]");
  process.exit(2);
}
// Confine the server to the folder that holds everything this run touches: the plug-in, the .pmap, and the export.
const commonDir = (a, b) => {
  const [x, y] = [a.split(sep), b.split(sep)];
  let i = 0;
  while (i < x.length && x[i] === y[i]) i++;
  return x.slice(0, i).join(sep) || sep;
};
const root = [resolve(import.meta.dirname, ".."), resolve(pmap), ...(rest[0] === "--demo" ? [resolve(rest[1])] : [])].reduce(commonDir);
const child = spawn(cli, ["mcp", "--automation-read-root", root, "--automation-write-root", root], { stdio: ["pipe", "pipe", "inherit"] });
let buffer = "";
const waiting = new Map();
child.stdout.on("data", (chunk) => {
  buffer += chunk;
  for (let nl = buffer.indexOf("\n"); nl >= 0; nl = buffer.indexOf("\n")) {
    const msg = JSON.parse(buffer.slice(0, nl));
    buffer = buffer.slice(nl + 1);
    waiting.get(msg.id)?.(msg);
  }
});
let nextId = 1;
const call = (method, params) =>
  new Promise((done) => {
    const id = nextId++;
    waiting.set(id, done);
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
  });
const tool = async (name, args) => {
  const r = await call("tools/call", { name, arguments: args });
  if (r.error) throw new Error(`${name}: ${JSON.stringify(r.error)}`);
  return r.result;
};

const init = await call("initialize", { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "road-builder-smoke", version: "0" } });
console.log("server:", init.result.serverInfo.name, init.result.serverInfo.version);
child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);
const listed = (await call("tools/list", {})).result.tools;
console.log(`${listed.length} tools, e.g.: ${listed.slice(0, 12).map((t) => t.name).join(", ")}`);
if (rest[0] === "--demo") {
  // The agent loop: open the road, add a junction by route metre, rebuild, export, all through MCP tools.
  const out = resolve(rest[1]);
  mkdirSync(dirname(out), { recursive: true });
  const run = async (id, params = {}) => JSON.stringify(await tool("command_run", { id, params })).slice(0, 160);
  // A server confined to folders has no plug-in store, so install the three plug-ins into this session first.
  for (const kind of ["importer", "filter", "exporter"]) {
    const wasm = resolve(import.meta.dirname, `../target/wasm32-unknown-unknown/release/road_builder_${kind}.wasm`);
    console.log(`install ${kind}`.padEnd(14), await run("plugin.install", { path: wasm, replace: true }));
  }
  console.log("open_file     ", JSON.stringify(await tool("open_file", { path: resolve(pmap) })).slice(0, 160));
  console.log("select.all    ", await run("select.all"));
  console.log("add_junction  ", await run("plugin.com.cosmicceo.roadbuilder.build.add_junction", { params: { s: 1500, kind: "cross", side: "both", label: "MCP Street" } }));
  console.log("select.all    ", await run("select.all"));
  console.log("Build Road    ", await run("plugin.run", { id: "com.cosmicceo.roadbuilder.build" }));
  console.log("export .pmap  ", await run("document.export", { format: "plugin.com.cosmicceo.roadbuilder.export", path: out }));
} else if (rest.length) {
  console.log(JSON.stringify(await tool(rest[0], JSON.parse(rest[1] ?? "{}"))).slice(0, 1200));
}
child.kill();
