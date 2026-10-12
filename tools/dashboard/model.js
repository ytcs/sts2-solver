// The dashboard's read of the run record (no DOM): model(live, events) -> the operator panels. The read contract is in serve.py's docstring.
"use strict";
(function (root) {
  const HDR = /\bA(\d+) F(\d+)\s+([A-Z]+)(?:\s+A(\d+))?\s+HP (-?\d+)\/(\d+)\s+G(\d+)(?:\s+pots\[([^\]]*)\])?/;
  function header(text) {
    const m = HDR.exec(text || "");
    return m ? { act: +m[1], floor: +m[2], char: m[3], asc: m[4] ? +m[4] : null, hp: +m[5], max: +m[6], gold: +m[7], pots: m[8] != null ? m[8].split(",").map((s) => s.trim()) : [] } : null;
  }
  const kindOf = (text) => (text || "").split("\n", 1)[0].split(" ")[0] || "";
  function options(text) {
    const out = [];
    for (const l of (text || "").split("\n")) { const m = /^(\d+) (.*)$/.exec(l); if (m) out.push({ n: +m[1], label: m[2] }); }
    return out;
  }
  function chosenLabel(state, choice) {
    const opts = options(state), c = String(choice || "").trim();
    if (/^\d+/.test(c)) { const o = opts.find((x) => x.n === +c.split(/\s+/)[0]); return o ? o.label : c; }
    if (c.startsWith("~")) { const t = c.slice(1).split(/\s+/)[0].toLowerCase(); const o = opts.find((x) => x.label.toLowerCase().includes(t)); return o ? o.label : c; }
    return c;
  }
  function splitReply(reply) {
    const r = String(reply || "");
    if (!r.startsWith("WARNING: ")) return { warning: null, body: r };
    const i = r.indexOf("\n");
    return i < 0 ? { warning: r, body: "" } : { warning: r.slice(0, i), body: r.slice(i + 1) };
  }
  // macro `why`: "rules: <IDs> ; numbers: ... ; judgment: ..." (labels in any order, any missing); free text otherwise
  function parseWhy(why) {
    if (!why) return null;
    const segs = String(why).split(/\s*;\s*(?=(?:rules|numbers|judgment):)/);
    const out = { text: String(why), rules: [], numbers: null, judgment: null, rest: null };
    let labelled = false;
    for (const s of segs) {
      const m = /^(rules|numbers|judgment):\s*([\s\S]*)$/.exec(s.trim());
      if (!m) { out.rest = [out.rest, s.trim()].filter(Boolean).join("; "); continue; }
      labelled = true;
      if (m[1] === "rules") {
        const sc = m[2].indexOf(";");
        out.rules = (sc < 0 ? m[2] : m[2].slice(0, sc)).split(/[\s,]+/).filter((x) => x && x !== "-");
        if (sc >= 0) out.rest = [out.rest, m[2].slice(sc + 1).trim()].filter(Boolean).join("; ");
      } else out[m[1]] = m[2].trim();
    }
    return labelled ? out : { text: String(why), rules: [], numbers: null, judgment: null, rest: String(why) };
  }
  // `price`: result {label: {check} | {win?, act, ready?, floors}} or null; search {label: [score, se, {factor: scored}, {factor: win}, {factor: HP multiple}?]}
  function priceRows(e) {
    if (!e || (!e.result && !e.search)) return [];
    const res = e.result || {}, srch = e.search || {};
    return (e.options || Object.keys(res)).map((k) => {
      const r = res[k] || {}, s = srch[k];
      const wins = (s && s[3]) || {}, scored = (s && s[2]) || {}, mult = (s && s[4]) || {};
      const factors = Object.keys(Object.keys(wins).length ? wins : scored).map((f) => {
        const w = wins[f] ?? null, p = scored[f] ?? null;
        return { name: f, win: w, scored: p, mult: mult[f] ?? null, floored: w != null && p != null && Math.abs(p - w) > 1e-9 };
      });
      return { option: k, check: s ? s[0] : r.check ?? null, se: s ? s[1] : null, factors, win: r.win ?? null, act: r.act ?? null, ready: r.ready ?? null, floors: r.floors ?? null };
    });
  }
  function rankKey(e) { return (e && e.ranked_by) || "check"; }
  function bestOption(e) {
    if (!e) return null;
    if (e.best) return e.best;
    const k = rankKey(e), rows = priceRows(e).filter((r) => r[k] != null);
    return rows.length ? rows.reduce((a, b) => (b[k] > a[k] ? b : a)).option : null;
  }
  function lastOf(events, kind, pred) { for (let i = events.length - 1; i >= 0; i--) { const e = events[i]; if (e.kind === kind && (!pred || pred(e))) return e; } return null; }
  function latestPrice(events, kind) {  // the price of the screen on view: none once a decision was made since
    for (let i = events.length - 1; i >= 0; i--) {
      const e = events[i];
      if (e.kind === "macro") return null;
      if (e.kind === "price" && e.screen === kind) return e;
    }
    return null;
  }
  function model(live, events) {
    live = live || {}; events = events || [];
    const macro = lastOf(events, "macro");
    const { warning, body } = splitReply(live.reply);
    const text = live.screen || (macro ? macro.state : "") || "";
    const kind = kindOf(text);
    const h = header(text) || header((lastOf(events, "macro", (e) => header(e.state)) || {}).state);
    const inFight = (kind === "COMBAT" || kind === "SELECT") && live.fight && live.fight.state && live.fight.state.combat_in_progress !== false;
    const fs = lastOf(events, "fight_start");
    const rel = lastOf(events, "relics");
    let relics = [];
    if (inFight && live.fight.state.relics) relics = live.fight.state.relics;
    else if (rel && (!fs || rel.t >= fs.t)) relics = rel.relics.map((id) => ({ id }));
    else if (fs && fs.scenario) relics = fs.scenario.relics || [];
    const sc = (live.fight && live.fight.scenario) || (fs && fs.scenario) || null;
    const start = inFight ? lastOf(events, "fight_start", (e) => e.id === live.fight.id) : null;
    const note = lastOf(events, "note"), target = lastOf(events, "target");
    let potion = null;
    if (live.potion) potion = { kind: "confirm", text: live.potion, line: body.split("\n").find((l) => l.startsWith("POTION: ")) || null };
    else if (!live.running && /^(turn|combat|potions)\b/.test(live.cmd || "")) {
      const l = body.split("\n").find((x) => x.startsWith("POTION PROPOSAL"));
      if (l) potion = { kind: "proposal", text: null, line: l };
    }
    return {
      run: live.run || null, t: live.t || null, cmd: live.cmd || null, reply: body, warning,
      running: live.running ? { cmd: live.running, since: live.t_running || null } : null,
      screen: { text, kind, t: live.screen ? live.t_screen || live.t : macro ? macro.t : null, live: !!live.screen, header: h },
      fight: inFight ? { state: live.fight.state, scenario: live.fight.scenario || null, id: live.fight.id, start } : null,
      relics, deck: sc ? sc.deck || [] : [], character: (h && h.char) || (sc && sc.character) || "",
      potion,
      decision: macro ? { t: macro.t, screen: macro.screen, header: header(macro.state), choice: macro.choice, label: chosenLabel(macro.state, macro.choice), why: parseWhy(macro.why), result: macro.result || null } : null,
      plan: note ? { t: note.t, text: note.text } : null,
      target: target ? { t: target.t, variant: target.variant || null } : null,
      price: latestPrice(events, kind),
    };
  }
  const API = { header, kindOf, options, chosenLabel, splitReply, parseWhy, priceRows, rankKey, bestOption, lastOf, latestPrice, model, HDR };
  if (typeof module !== "undefined" && module.exports) module.exports = API; else root.DM = API;
})(typeof window !== "undefined" ? window : globalThis);
