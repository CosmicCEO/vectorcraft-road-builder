// Tiny client for a running VectorCraft app's control port (start the app with: open -a VectorCraft --args --control 7979).
// Usage: node scripts/vc.mjs <method> [json-params]        e.g.  node scripts/vc.mjs engine.execute '{"command":"select.all"}'
// Prints the reply's result (or exits 1 with the error).
import net from "node:net";

const [method, raw] = process.argv.slice(2);
if (!method) {
  console.error("usage: vc.mjs <method> [json-params]");
  process.exit(2);
}
const port = Number(process.env.VECTORCRAFT_CONTROL_PORT ?? 7979);
const socket = net.connect(port, "127.0.0.1");
let buffer = "";
socket.on("error", (e) => {
  console.error(`cannot reach VectorCraft on port ${port}: ${e.message}`);
  process.exit(1);
});
socket.on("connect", () => socket.write(`${JSON.stringify({ id: 1, method, params: raw ? JSON.parse(raw) : {} })}\n`));
socket.on("data", (chunk) => {
  buffer += chunk;
  if (!buffer.includes("\n")) return;
  const reply = JSON.parse(buffer.split("\n")[0]);
  socket.end();
  if (!reply.ok) {
    console.error(reply.error);
    process.exit(1);
  }
  console.log(JSON.stringify(reply.result));
});
