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
