// A stand-in for VectorCraft's control port: just enough of the protocol for the panel, and it records every command it is sent.
import { writeFileSync } from "node:fs";
import net from "node:net";
import { PLUGIN_ID, SET_CROSSING, SET_CROSS_SECTION, SET_JUNCTION } from "../lib/doc.mjs";

export async function fakeVectorCraft({ paths = [], selection = [], spec = { segments: {} }, hasDocument = true } = {}) {
  const doc = { paths: paths.map((p) => ({ kind: "Path", ...p })), selection: [...selection], spec: structuredClone(spec), revision: 1, calls: [], layers: [{ id: 1000, name: "Layer 1", visible: true, locked: false, parent: null }], nextLayer: 1001 };
  const tree = (parent = null) => doc.layers.filter((l) => l.parent === parent).map((l) => ({ id: l.id, kind: "Layer", name: l.name, visible: l.visible, locked: l.locked, children: tree(l.id) }));
  const reply = (socket, id, result) => socket.write(`${JSON.stringify({ id, ok: true, result })}\n`);
  const fail = (socket, id, error) => socket.write(`${JSON.stringify({ id, ok: false, error })}\n`);

  const server = net.createServer((socket) => {
    let buf = "";
    socket.setEncoding("utf8");
    socket.on("data", (chunk) => {
      buf += chunk;
      for (let i = buf.indexOf("\n"); i >= 0; i = buf.indexOf("\n")) {
        const { id, method, params } = JSON.parse(buf.slice(0, i));
        buf = buf.slice(i + 1);
        if (!hasDocument && method === "document.inspect") return fail(socket, id, "no active document");
        if (method === "document.inspect") {
          if (params.ids) return reply(socket, id, { nodes: doc.paths.filter((p) => params.ids.includes(p.id)) });
          if (params.kind) return reply(socket, id, { nodes: doc.paths.map((p) => ({ ...p, path: p.path ?? [1000] })) });
          return reply(socket, id, { revision: doc.revision, fileInfoTitle: "Untitled", selection: doc.selection, layers: tree() });
        }
        if (method !== "engine.execute") return fail(socket, id, `unknown method ${method}`);
        const { command, params: p = {} } = params;
        doc.calls.push({ command, params: p });
        if (command === "select.set") doc.selection = [...p.ids];
        else if (command === "layer.new" || command === "layer.newSublayer") {
          const layer = { id: doc.nextLayer++, name: p.name, visible: true, locked: false, parent: command === "layer.newSublayer" ? p.parent : null };
          if (p.top) doc.layers.unshift(layer);
          else doc.layers.push(layer);
          return reply(socket, id, { id: layer.id });
        } else if (command === "layer.move") {
          for (const x of doc.paths.filter((q) => p.ids.includes(q.id))) x.path = [p.target];
          for (const l of doc.layers.filter((q) => p.ids.includes(q.id))) {
            doc.layers.splice(doc.layers.indexOf(l), 1);
            doc.layers.splice(doc.layers.findIndex((q) => q.id === p.target), 0, l);
          }
        } else if (command === "layer.setProps") {
          const l = doc.layers.find((q) => q.id === p.id);
          if (p.visible !== undefined) l.visible = p.visible;
          if (p.locked !== undefined) l.locked = p.locked;
        } else if (command === "select.all") doc.selection = doc.paths.map((x) => x.id);
        else if (command === SET_CROSS_SECTION) {
          const args = p.params;
          const sel = doc.paths.filter((x) => doc.selection.includes(x.id));
          for (const x of sel) {
            let seg = x.name.startsWith("road.") ? x.name.slice(5) : null;
            if (!seg || args.scope === "selected") {
              seg = `r${Object.keys(doc.spec.segments).length + 1}`;
              x.name = `road.${seg}`;
            }
            doc.spec.segments[seg] = { cross_section: args.cross_section, anchor: "auto" };
          }
          doc.revision++;
        } else if (command === SET_JUNCTION) {
          doc.spec.junctions ??= {};
          for (const x of doc.paths.filter((q) => doc.selection.includes(q.id))) {
            let id = x.name.startsWith("junction.") ? x.name.slice(9) : null;
            if (!id) {
              id = `j${Object.keys(doc.spec.junctions).length + 1}`;
              x.name = `junction.${id}`;
            }
            doc.spec.junctions[id] = JSON.parse(p.params.description);
          }
          doc.revision++;
        } else if (command === SET_CROSSING) {
          const roads = [...new Set(doc.paths.filter((q) => doc.selection.includes(q.id)).map((q) => q.name.slice(5)))];
          doc.spec.crossings ??= {};
          const known = Object.entries(doc.spec.crossings).find(([, c]) => roads.every((r) => c.roads.includes(r)))?.[0];
          doc.spec.crossings[known ?? `k${Object.keys(doc.spec.crossings).length + 1}`] = { ...JSON.parse(p.params.description), roads };
          doc.revision++;
        } else if (command === "file.saveCopy") {
          const body = JSON.stringify({ document: { unknown: { pluginData: { [PLUGIN_ID]: doc.spec } } } });
          writeFileSync(p.path, `${body}\n\u0000VCBLOBS\u0000%PDF-1.7 binary preview`, "latin1");
          return reply(socket, id, { path: p.path });
        } else if (command === "plugin.run") doc.revision++;
        reply(socket, id, { ok: true });
      }
    });
  });
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  return { port: server.address().port, doc, close: () => server.close() };
}
