#!/usr/bin/env node
// The companion panel: a small local web page beside VectorCraft that follows your selection, shows the road's elements,
// takes a description in words, and draws it. Zero dependencies.
//
//   open -a VectorCraft --args --control 7979      (VectorCraft with its control port; loopback only)
//   node panel/server.mjs [--control 7979] [--port 8787] [--agent claude]
//   then open http://127.0.0.1:8787
import { createServer as httpServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Control } from "./lib/control.mjs";
import { Panel } from "./lib/panel.mjs";
import { loadProviders } from "./lib/providers.mjs";

const PUBLIC = resolve(import.meta.dirname, "public");
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8" };

/** The HTTP side. Only answers requests that name this server (Host) and, for changes, that come from its own page (Origin). */
export function createServer({ panel, port }) {
  const hosts = new Set([`127.0.0.1:${port}`, `localhost:${port}`]);
  const json = (res, code, body) => {
    res.writeHead(code, { "content-type": "application/json", "cache-control": "no-store" });
    res.end(JSON.stringify(body));
  };
  const readBody = (req) =>
    new Promise((done, fail) => {
      let t = "";
      req.on("data", (d) => {
        t += d;
        if (t.length > 100_000) fail(new Error("request too large"));
      });
      req.on("end", () => {
        try {
          done(t ? JSON.parse(t) : {});
        } catch {
          fail(new Error("the request was not valid JSON"));
        }
      });
    });

  const actions = {
    propose: (b) => panel.propose(String(b.description ?? "").slice(0, 2000), b.agent),
    edit: (b) => panel.edit(b.op),
    text: (b) => panel.setText(String(b.text ?? "").slice(0, 1000)),
    draw: (b) => panel.draw({ scope: b.scope }),
    cancel: () => panel.cancel(),
    selectline: () => panel.selectLine(),
    mode: (b) => panel.setMode(String(b.mode)),
    match: () => panel.match(),
    junction: (b) => panel.editJunction(b.patch ?? {}),
    crossing: (b) => panel.editCrossing(b.patch ?? {}),
    hub: (b) => panel.editHub(b.patch ?? {}),
    transition: (b) => panel.editTransition(b.patch ?? {}),
    removetransition: (b) => panel.removeTransition(Number(b.index)),
    agent: (b) => panel.setAgent(String(b.id)),
  };

  return httpServer(async (req, res) => {
    if (!hosts.has(req.headers.host ?? "")) return json(res, 403, { error: "unexpected Host header" });
    const url = new URL(req.url, `http://${req.headers.host}`);
    try {
      if (req.method === "GET" && url.pathname === "/events") {
        res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-store", connection: "keep-alive" });
        const send = (s) => res.write(`data: ${JSON.stringify(s)}\n\n`);
        send(panel.snapshot());
        panel.on("change", send);
        req.on("close", () => panel.off("change", send));
        return;
      }
      if (req.method === "GET" && url.pathname === "/state") return json(res, 200, panel.snapshot());
      if (req.method === "POST" && url.pathname.startsWith("/api/")) {
        const origin = req.headers.origin;
        if ((origin && !hosts.has(new URL(origin).host)) || !String(req.headers["content-type"] ?? "").startsWith("application/json")) return json(res, 403, { error: "requests must come from the panel page" });
        const action = actions[url.pathname.slice(5)];
        if (!action) return json(res, 404, { error: "unknown action" });
        await action(await readBody(req));
        return json(res, 200, panel.snapshot());
      }
      if (req.method === "GET") {
        const file = url.pathname === "/" ? "index.html" : url.pathname.slice(1);
        if (!/^[\w.-]+$/.test(file) || !TYPES[extname(file)]) return json(res, 404, { error: "not found" });
        const body = await readFile(resolve(PUBLIC, file));
        res.writeHead(200, { "content-type": TYPES[extname(file)], "cache-control": "no-store" });
        return res.end(body);
      }
      json(res, 405, { error: "method not allowed" });
    } catch (e) {
      json(res, e.code === "ENOENT" ? 404 : 400, { error: e.message });
    }
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const arg = (name, fallback) => {
    const i = process.argv.indexOf(`--${name}`);
    return i > 0 ? process.argv[i + 1] : fallback;
  };
  const port = Number(arg("port", 8787));
  const providers = loadProviders();
  if (arg("agent")) providers.default = arg("agent");
  const panel = new Panel({ control: new Control({ port: Number(arg("control", 7979)) }), providers });
  createServer({ panel, port }).listen(port, "127.0.0.1", () => {
    console.log(`Road Builder panel: http://127.0.0.1:${port}   (VectorCraft control port ${arg("control", 7979)}, agent ${providers.default})`);
    panel.start();
  });
}
