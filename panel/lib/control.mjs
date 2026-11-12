// Client for a running VectorCraft's control port (start the app with:  open -a VectorCraft --args --control 7979).
// One JSON request per line, one JSON reply per line. Loopback only, no authentication: only open it while you use it.
import net from "node:net";

export class Control {
  constructor({ port = 7979, host = "127.0.0.1" } = {}) {
    this.port = port;
    this.host = host;
    this.socket = null;
    this.buffer = "";
    this.waiting = new Map();
    this.nextId = 1;
  }

  get connected() {
    return !!this.socket && !this.socket.destroyed;
  }

  connect() {
    if (this.connected) return Promise.resolve();
    return new Promise((done, fail) => {
      const s = net.connect(this.port, this.host);
      s.setEncoding("utf8");
      s.once("error", (e) => {
        this.socket = null;
        fail(new Error(`VectorCraft is not answering on port ${this.port}. Start it with: open -a VectorCraft --args --control ${this.port} (${e.code ?? e.message})`));
      });
      s.once("connect", () => {
        this.socket = s;
        s.on("data", (chunk) => this.#data(chunk));
        s.on("close", () => this.#closed());
        done();
      });
    });
  }

  #data(chunk) {
    this.buffer += chunk;
    for (let i = this.buffer.indexOf("\n"); i >= 0; i = this.buffer.indexOf("\n")) {
      const line = this.buffer.slice(0, i);
      this.buffer = this.buffer.slice(i + 1);
      if (!line.trim()) continue;
      let msg;
      try {
        msg = JSON.parse(line);
      } catch {
        continue;
      }
      const w = this.waiting.get(msg.id);
      if (!w) continue;
      this.waiting.delete(msg.id);
      clearTimeout(w.timer);
      msg.ok ? w.done(msg.result) : w.fail(new Error(String(msg.error)));
    }
  }

  #closed() {
    this.socket = null;
    for (const [id, w] of this.waiting) {
      clearTimeout(w.timer);
      w.fail(new Error("VectorCraft closed the connection"));
      this.waiting.delete(id);
    }
  }

  async call(method, params = {}, timeoutMs = 60_000) {
    await this.connect();
    return new Promise((done, fail) => {
      const id = this.nextId++;
      const timer = setTimeout(() => {
        this.waiting.delete(id);
        fail(new Error(`VectorCraft did not answer ${method} within ${timeoutMs / 1000} s`));
      }, timeoutMs);
      this.waiting.set(id, { done, fail, timer });
      this.socket.write(`${JSON.stringify({ id, method, params })}\n`);
    });
  }

  /** Runs an engine command (what menu items and plug-in commands are). */
  run(command, params = {}, timeoutMs) {
    return this.call("engine.execute", { command, params }, timeoutMs);
  }

  close() {
    this.socket?.destroy();
  }
}
