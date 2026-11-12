// The instructions every agent gets. Kept compact on purpose: a small on-device model must be able to follow them.

export function systemPrompt(lib) {
  const els = lib.elements.map((e) => `${e.id} (${e.width} m${e.directional ? ", has a direction" : ""}): ${e.label}`).join("\n");
  return `You turn a plain-language description of a road's cross-section into one line of "Road Builder shorthand". Reply with that one line and nothing else: no quotes, no explanation, no code fence.

SHORTHAND
- Items are separated by " | " and read LEFT TO RIGHT as seen facing the direction given after "facing".
- Start with "facing N:" (or S, E, W) when the description says which way the road runs, and list from the left side to the right side as seen facing that way. If no orientation is given, leave "facing" out and list left to right along the drawn line.
- An item is an element name, then optional pieces in any order: a direction (N S E W, or fwd / back) for elements that have one (travel lanes, bike lanes, turn pockets), a width ("3" = metres, "10ft"), a repeat ("x2").
- US right-hand traffic: facing north, the lanes on the LEFT (west) side run south (S) and the lanes on the RIGHT (east) side run north (N).
- Use only these elements (default width in brackets). If something is not listed, use the closest one; never invent a name:
${els}

EXAMPLES
Description: road runs north-south; 1 bike lane, 1 apron, 2 south lanes, a 3 m grassy curbed margin, 2 north lanes, apron, bike lane
facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane

Description: a quiet two-lane street with parking on both sides
parking-lane | lane fwd | lane back | parking-lane

EDITING
When a current cross-section is given, apply the requested change to it and return the COMPLETE updated line. Keep everything that was not mentioned exactly as it was (same order, same widths). "Delete/remove X" removes that item; "add X" inserts it where it belongs in left-to-right order; "make X wider/narrower" sets that item's width.`;
}

export function userPrompt({ description, current }) {
  return current ? `Current cross-section:\n${current}\n\nChange requested: ${description}` : `Description: ${description}`;
}

/** Pulls the shorthand line out of whatever the model said (it may wrap it in quotes or a code fence despite instructions). */
export function extractLine(reply) {
  const cleaned = String(reply).replace(/```[a-z]*\n?/gi, "").trim();
  const lines = cleaned.split("\n").map((l) => l.trim().replace(/^["'`]+|["'`]+$/g, "")).filter(Boolean);
  return lines.find((l) => /^facing\b/i.test(l)) ?? lines.find((l) => l.includes("|")) ?? lines[0] ?? "";
}

export function junctionSystemPrompt(lib) {
  const els = lib.elements.map((e) => e.id).join(", ");
  return `You turn a plain-language description of a road junction into one JSON object. Reply with the JSON object only: no explanation, no code fence.

The junction is where a short connecting line joins two road pieces; the main road carries on through it. The JSON describes what crosses it:
{"legs": [{"side": "...", "cross_section": "...", "length": 40}], "control": "none|signal|stop", "crosswalks": false, "radius": 6}
- legs: 0, 1 or 2 side roads. No legs means just carry the road across the gap. One leg is a T-junction, two legs (one each side) a four-way junction.
- side: "west", "east", "north" or "south" if the description gives compass directions, otherwise "left" or "right" as seen travelling along the main road. A leg must cross the main road, so never use the direction the main road runs.
- cross_section: the side road written in the Road Builder shorthand: elements separated by " | ", each an element name with optional direction (fwd, back), width in metres, or repeat (x2). Elements: ${els}. A two-lane road is "lane fwd | lane back"; a "rural road" is two lanes, possibly with shoulders ("shoulder | lane fwd | lane back | shoulder").
- length: how far the leg is drawn, in metres (default 40).
- control: "signal" for traffic lights or signals, "stop" for stop signs on the side roads, otherwise "none".
- crosswalks: true if there are crosswalks, cross walks or pedestrian crossings.
- radius: corner radius in metres (default 6).
Do not add a turn pocket or turn lane here: describe only the legs, control and crosswalks.

EXAMPLE
Description: This is a 4 way intersection. The west leg is a two lane rural road, the east is a two lane rural road. There are traffic lights and cross walks at each side.
{"legs": [{"side": "west", "cross_section": "lane fwd | lane back", "length": 40}, {"side": "east", "cross_section": "lane fwd | lane back", "length": 40}], "control": "signal", "crosswalks": true, "radius": 6}

EDITING
When a current junction is given, apply the requested change and return the COMPLETE updated object, keeping everything not mentioned exactly as it was.`;
}

export function junctionUserPrompt({ description, current }) {
  return current ? `Current junction:\n${JSON.stringify(current)}\n\nChange requested: ${description}` : `Description: ${description}`;
}

/** Pulls the first balanced JSON object out of a reply (models sometimes add words or a code fence). */
export function extractJson(reply) {
  const text = String(reply).replace(/```[a-z]*\n?/gi, "");
  const start = text.indexOf("{");
  if (start < 0) return "";
  let depth = 0;
  let inString = false;
  for (let i = start; i < text.length; i++) {
    const c = text[i];
    if (inString) {
      if (c === "\\") i++;
      else if (c === '"') inString = false;
    } else if (c === '"') inString = true;
    else if (c === "{") depth++;
    else if (c === "}" && --depth === 0) return text.slice(start, i + 1);
  }
  return "";
}
