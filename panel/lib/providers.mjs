// Agent providers: anything that turns text into the cross-section shorthand. The panel (not the model) validates the answer with the
// native CLI, shows what would change, and applies it, so a provider never touches VectorCraft and any model can be plugged in.
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { resolve } from "node:path";
import { library, parseCrossSection, parseCrossing, parseJunction } from "./cli.mjs";
import { crossingSystemPrompt, crossingUserPrompt, extractJson, extractLine, junctionSystemPrompt, junctionUserPrompt, systemPrompt, userPrompt } from "./prompt.mjs";

const here = import.meta.dirname;

/** The built-in providers.json, with the user's own (ROAD_BUILDER_PROVIDERS or ~/.config/vectorcraft-road-builder/providers.json) on top. */
export function loadProviders(extraPath) {
  const base = JSON.parse(readFileSync(resolve(here, "../providers.json"), "utf8"));
  const mine = extraPath ?? process.env.ROAD_BUILDER_PROVIDERS ?? resolve(homedir(), ".config/vectorcraft-road-builder/providers.json");
  if (!existsSync(mine)) return base;
  const extra = JSON.parse(readFileSync(mine, "utf8"));
  return { default: extra.default ?? base.default, providers: { ...base.providers, ...extra.providers } };
}

function run(command, args, { input, cwd, timeoutMs = 90_000 } = {}) {
  return new Promise((done, fail) => {
    const child = spawn(command, args, { cwd: cwd ?? tmpdir(), stdio: ["pipe", "pipe", "pipe"] });
    let out = "";
    let err = "";
    const timer = setTimeout(() => {
      child.kill();
      fail(new Error(`${command} took longer than ${timeoutMs / 1000} s`));
    }, timeoutMs);
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (err += d));
    child.on("error", (e) => {
      clearTimeout(timer);
      fail(e.code === "ENOENT" ? new Error(`${command} is not installed or not on the PATH`) : e);
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      code === 0 ? done(out) : fail(new Error(`${command} failed (${code}): ${(err || out).trim().slice(0, 300)}`));
    });
    child.stdin.end(input ?? "");
  });
}

/** Asks one provider once. Returns the model's raw reply text. */
export async function ask(cfg, { system, user, request }) {
  switch (cfg.kind) {
    case "claude": {
      // Text only: no tools, no MCP, no project files, no user hooks; a fresh one-turn conversation on the existing Claude login.
      const args = ["-p", user, "--model", cfg.model ?? "haiku", "--tools", "", "--strict-mcp-config", "--no-session-persistence", "--setting-sources", "project", "--system-prompt", system, "--output-format", "json"];
      const j = JSON.parse(await run(cfg.command ?? "claude", args));
      if (j.is_error) throw new Error(`Claude: ${j.result ?? "error"}`);
      return j.result;
    }
    case "cli": {
      // Any program: reads the request as JSON on stdin, prints the cross-section (plain text, or JSON with a cross_section field).
      // A command starting with ./ is relative to the repository (e.g. ./target/apple-fm-agent).
      const command = cfg.command.startsWith("./") ? resolve(here, "../..", cfg.command) : cfg.command;
      const out = (await run(command, cfg.args ?? [], { input: JSON.stringify({ system, user, ...request }) })).trim();
      try {
        const j = JSON.parse(out);
        return typeof j === "string" ? j : j.cross_section ?? out;
      } catch {
        return out;
      }
    }
    case "http": {
      // OpenAI-compatible chat endpoint: a hosted model, or a local one (Ollama, LM Studio, llama.cpp server, ...).
      const key = cfg.api_key_env ? process.env[cfg.api_key_env] : undefined;
      if (cfg.api_key_env && !key) throw new Error(`set the ${cfg.api_key_env} environment variable for this provider`);
      const res = await fetch(`${cfg.base_url.replace(/\/$/, "")}/chat/completions`, {
        method: "POST",
        headers: { "content-type": "application/json", ...(key ? { authorization: `Bearer ${key}` } : {}) },
        body: JSON.stringify({ model: cfg.model, temperature: 0, messages: [{ role: "system", content: system }, { role: "user", content: user }] }),
        signal: AbortSignal.timeout(90_000),
      });
      if (!res.ok) throw new Error(`${cfg.base_url} answered ${res.status}: ${(await res.text()).slice(0, 200)}`);
      return (await res.json()).choices?.[0]?.message?.content ?? "";
    }
    case "function":
      return cfg.fn({ system, user, request });
    default:
      throw new Error(`unknown provider kind "${cfg.kind}" (use claude, cli, http)`);
  }
}

/**
 * Turns a description into a validated cross-section. The provider answers, the CLI checks the answer, and if it is not valid the
 * provider gets the exact error once to fix it. Returns {ok: true, ...parsed, attempts} or {ok: false, error, attempts}.
 */
export async function propose({ description, current = null, provider, maxAttempts = 2, lib }) {
  lib ??= await library();
  const system = systemPrompt(lib);
  const request = { description, current };
  let user = userPrompt(request);
  const attempts = [];
  for (let n = 1; n <= maxAttempts; n++) {
    let reply;
    try {
      reply = await ask(provider, { system, user, request });
    } catch (e) {
      return { ok: false, error: e.message, attempts };
    }
    const line = extractLine(reply);
    const parsed = await parseCrossSection(line);
    attempts.push({ reply: String(reply).slice(0, 400), line, ok: parsed.ok, error: parsed.error });
    if (parsed.ok) return { ...parsed, attempts };
    user = `${userPrompt(request)}\n\nYour previous answer was: ${line}\nIt was rejected: ${parsed.error}\nReply again with a corrected single line.`;
  }
  return { ok: false, error: `the agent could not produce a valid cross-section: ${attempts.at(-1)?.error}`, attempts };
}

/**
 * Turns a description of a junction into a validated junction (legs, control, crosswalks), the same way `propose` does for a road.
 * `current` is the junction's present description (an object) when editing. Returns {ok: true, junction, summary, attempts} or {ok: false, error, attempts}.
 */
export async function proposeJunction({ description, current = null, provider, maxAttempts = 2, lib }) {
  lib ??= await library();
  const system = junctionSystemPrompt(lib);
  const request = { description, current, kind: "junction" };
  let user = junctionUserPrompt(request);
  const attempts = [];
  for (let n = 1; n <= maxAttempts; n++) {
    let reply;
    try {
      reply = await ask(provider, { system, user, request });
    } catch (e) {
      return { ok: false, error: e.message, attempts };
    }
    const json = extractJson(reply);
    const parsed = json ? await parseJunction(json) : { ok: false, error: "the answer had no JSON object in it" };
    attempts.push({ reply: String(reply).slice(0, 600), json, ok: parsed.ok, error: parsed.error });
    if (parsed.ok) return { ...parsed, attempts };
    user = `${junctionUserPrompt(request)}\n\nYour previous answer was: ${json || String(reply).slice(0, 300)}\nIt was rejected: ${parsed.error}\nReply again with a corrected JSON object only.`;
  }
  return { ok: false, error: `the agent could not produce a valid junction: ${attempts.at(-1)?.error}`, attempts };
}

/**
 * Turns a description of how two crossing roads meet (lighted, stops, overpass, roundabout ...) into a validated intersection.
 * `roads` are the two road ids; `current` is the present description (an object) when editing. Returns {ok: true, crossing, summary, attempts} or {ok: false, error, attempts}.
 */
export async function proposeCrossing({ description, current = null, roads, provider, maxAttempts = 2 }) {
  const system = crossingSystemPrompt();
  const request = { description, current, roads, kind: "crossing" };
  let user = crossingUserPrompt(request);
  const attempts = [];
  for (let n = 1; n <= maxAttempts; n++) {
    let reply;
    try {
      reply = await ask(provider, { system, user, request });
    } catch (e) {
      return { ok: false, error: e.message, attempts };
    }
    const json = extractJson(reply);
    let parsed = { ok: false, error: "the answer had no JSON object in it" };
    if (json) {
      try {
        parsed = await parseCrossing(JSON.stringify({ ...JSON.parse(json), roads }));
      } catch {
        parsed = { ok: false, error: "the answer was not valid JSON" };
      }
    }
    attempts.push({ reply: String(reply).slice(0, 600), json, ok: parsed.ok, error: parsed.error });
    if (parsed.ok) return { ...parsed, attempts };
    user = `${crossingUserPrompt(request)}\n\nYour previous answer was: ${json || String(reply).slice(0, 300)}\nIt was rejected: ${parsed.error}\nReply again with a corrected JSON object only.`;
  }
  return { ok: false, error: `the agent could not produce a valid intersection: ${attempts.at(-1)?.error}`, attempts };
}
