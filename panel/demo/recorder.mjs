// Captures the panel (browser window, by window id) and the VectorCraft window side by side at each step, and cuts the steps into a video.
// The panel must be open in Safari at http://127.0.0.1:8787/?zoom=0.6 (the zoom makes the whole panel fit the window).
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

export function createRecorder({ vc, out }) {
  mkdirSync(out, { recursive: true });
  const browser = JSON.parse(execFileSync("swift", [resolve(import.meta.dirname, "windows.swift"), "Safari"]).toString()).find((w) => /Road Builder/.test(w.name));
  if (!browser) throw new Error('Open "http://127.0.0.1:8787/?zoom=0.6" in Safari first');
  const steps = [];
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  return {
    async shot(title, subtitle) {
      await wait(1200); // let the page redraw
      const n = String(steps.length + 1).padStart(2, "0");
      const [panel, win, screen] = [`${out}/panel-${n}.png`, `${out}/vc-${n}.png`, `${out}/screen-${n}.png`];
      execFileSync("screencapture", ["-x", "-o", "-l", String(browser.id), screen]);
      // Crop away the toolbar and tab bar (about 170 px at 2x) and keep the panel's column, centred in the window.
      execFileSync("python3", ["-c", `from PIL import Image; im = Image.open("${screen}"); w, h = im.size; im.crop((w // 2 - 360, 170, w // 2 + 360, h)).save("${panel}")`]);
      await vc.call("ui.screenshot", { path: win });
      execFileSync("python3", [resolve(import.meta.dirname, "compose.py"), panel, win, `${out}/step-${n}.png`, title, subtitle]);
      steps.push(`${out}/step-${n}.png`);
      console.log(`${n}. ${title}`);
    },
    video(name) {
      writeFileSync(`${out}/list.txt`, steps.map((f) => `file '${f}'\nduration 5`).join("\n") + `\nfile '${steps.at(-1)}'\n`);
      execFileSync("ffmpeg", ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", `${out}/list.txt`, "-vf", "scale=1920:-2,format=yuv420p", "-c:v", "libx264", "-crf", "24", `${out}/${name}`]);
      console.log(`\nvideo: ${out}/${name}`);
    },
  };
}
