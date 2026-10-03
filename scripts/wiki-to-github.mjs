#!/usr/bin/env node
// Turns the website wiki (docs/wiki) into Markdown for the GitHub wiki
// (https://github.com/shankar-sachin/canopy/wiki). The website stays the
// source of truth: edit docs/wiki, run this, push the output to the wiki repo.
//
//   node scripts/wiki-to-github.mjs <out-dir>
//
// Needs Node 20 and nothing from npm. Writes one .md file per page, plus
// _Sidebar.md and _Footer.md. Skipped: the old terminal-frame screenshots
// (colored <pre> frames; the site has them), which become a link to the page.
// scripts/sync-github-wiki.sh does the whole job (clone, convert, push).
import { mkdirSync, readFileSync, rmSync, writeFileSync, readdirSync } from "node:fs";
import { dirname, join, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { GUIDES, PAGES } from "./wiki-pages.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SITE = "https://shankar-sachin.github.io/canopy/";
const out = process.argv[2];
if (!out) { console.error("usage: node scripts/wiki-to-github.mjs <out-dir>"); process.exit(2); }

// ---------------------------------------------------------------- names

/** The wiki page name for a docs/wiki-relative file ("desktop/tour.html"). */
function pageName(file) {
  const parts = file.replace(/\.html$/, "").split("/");
  if (parts.length === 1) return parts[0] === "index" ? "Home" : cap(parts[0]);
  const [guide, rest] = parts;
  return `${cap(guide)}-${rest === "index" ? "guide" : rest}`;
}
const cap = (s) => s[0].toUpperCase() + s.slice(1);
const byFile = new Map(PAGES.map((p) => [p.file, p]));

// GitHub's heading anchors: lowercase, drop punctuation, spaces to dashes.
const slug = (t) => t.toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").trim().replace(/\s/g, "-");

// ---------------------------------------------------------------- HTML

const VOID = new Set(["img", "br", "hr", "input", "meta", "link"]);
const ENT = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", mdash: "—", ndash: "–", hellip: "…", rarr: "→", larr: "←", uarr: "↑", darr: "↓", middot: "·", times: "×", ldquo: "“", rdquo: "”", lsquo: "‘", rsquo: "’" };
const decode = (s) =>
  s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (m, e) => {
    if (e[0] === "#") return String.fromCodePoint(e[1] === "x" || e[1] === "X" ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
    return ENT[e.toLowerCase()] ?? m;
  });

function parse(html) {
  const root = { tag: "#root", attrs: {}, kids: [] };
  const stack = [root];
  const re = /<!--[\s\S]*?-->|<\/([a-zA-Z0-9]+)\s*>|<([a-zA-Z0-9]+)((?:\s+[^\s=>\/]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*)\s*\/?>|([^<]+|<)/g;
  let m;
  while ((m = re.exec(html))) {
    const top = stack[stack.length - 1];
    if (m[0].startsWith("<!--")) continue;
    if (m[1]) {
      const t = m[1].toLowerCase();
      for (let i = stack.length - 1; i > 0; i--) if (stack[i].tag === t) { stack.length = i; break; }
    } else if (m[2]) {
      const tag = m[2].toLowerCase();
      const attrs = {};
      for (const a of m[3].matchAll(/([^\s=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g)) attrs[a[1].toLowerCase()] = decode(a[2] ?? a[3] ?? a[4] ?? "");
      const node = { tag, attrs, kids: [] };
      top.kids.push(node);
      if (!VOID.has(tag)) stack.push(node);
    } else if (m[4]) top.kids.push({ tag: "#text", text: m[4] });
  }
  return root;
}

const cls = (n) => (n.attrs?.class || "").split(/\s+/);
const text = (n) => (n.tag === "#text" ? decode(n.text) : n.kids ? n.kids.map(text).join("") : "");
const squash = (s) => s.replace(/\s+/g, " ");

// ---------------------------------------------------------------- convert

let ctxFile = "";
const anchors = new Map(); // wiki page name -> Map(id -> github slug)

function link(href) {
  if (!href) return "";
  if (/^(https?:|mailto:)/.test(href)) return href;
  const [path, hash] = href.split("#");
  if (!path) return hash ? "#" + (anchors.get(pageName(ctxFile))?.get(hash) ?? hash) : "";
  const target = posix.normalize(posix.join(posix.dirname(ctxFile), path));
  if (target.endsWith(".html") && byFile.has(target)) {
    const name = pageName(target);
    return name + (hash ? "#" + (anchors.get(name)?.get(hash) ?? hash) : "");
  }
  // Anything else lives on the website, relative to docs/.
  const site = posix.normalize(posix.join("wiki", posix.dirname(ctxFile), path));
  return SITE + site + (hash ? "#" + hash : "");
}

function inline(nodes) {
  let s = "";
  for (const n of nodes) {
    if (n.tag === "#text") { s += squash(decode(n.text)).replace(/</g, "&lt;"); continue; }
    const c = cls(n);
    switch (n.tag) {
      case "a": {
        if (c.includes("hash")) break;
        const t = inline(n.kids).trim();
        s += t ? `[${t}](${link(n.attrs.href)})` : "";
        break;
      }
      case "b": case "strong": { const t = inline(n.kids); s += t.trim() ? `**${t.trim()}**` : t; break; }
      case "i": case "em": { const t = inline(n.kids); s += t.trim() ? `*${t.trim()}*` : t; break; }
      case "code": { const t = text(n); s += t.includes("`") ? `<code>${esc(t)}</code>` : "`" + t + "`"; break; }
      case "kbd": s += `<kbd>${esc(text(n))}</kbd>`; break;
      case "br": s += "  \n"; break;
      case "img": s += img(n); break;
      case "span": case "small": case "mark": s += inline(n.kids); break;
      default: s += inline(n.kids || []);
    }
  }
  return s;
}
const esc = (t) => t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const cell = (s) => s.replace(/\|/g, "\\|").replace(/\n+/g, " <br> ").trim();

function img(n) {
  const src = n.attrs.src || "";
  const site = posix.normalize(posix.join("wiki", posix.dirname(ctxFile), src));
  return `![${(n.attrs.alt || "").replace(/[\[\]]/g, "")}](${/^https?:/.test(src) ? src : SITE + site})`;
}

const BLOCKS = new Set(["p", "ul", "ol", "pre", "table", "h1", "h2", "h3", "h4", "figure", "div", "blockquote", "hr"]);

function blocks(nodes, depth = 0) {
  const parts = [];
  let run = [];
  const flush = () => {
    const t = inline(run).trim();
    if (t) parts.push(t);
    run = [];
  };
  for (const n of nodes) {
    if (n.tag === "#text" || !BLOCKS.has(n.tag)) { run.push(n); continue; }
    flush();
    const b = block(n, depth);
    if (b) parts.push(b);
  }
  flush();
  return parts.join("\n\n");
}

function list(n, depth) {
  const ordered = n.tag === "ol";
  let i = 0;
  const items = n.kids.filter((k) => k.tag === "li").map((li) => {
    i++;
    const mark = ordered ? `${i}. ` : "- ";
    const pad = " ".repeat(mark.length);
    // Text before any nested list is the item; nested lists and blocks follow.
    const body = blocks(li.kids, depth + 1).split("\n").map((l, j) => (j === 0 ? l : l ? pad + l : l)).join("\n");
    return mark + body.replace(/\n\n(?=\s*(?:- |\d+\. ))/g, "\n");
  });
  return items.join("\n");
}

function table(n) {
  const rows = [];
  let headed = false;
  const walk = (x) => x.kids?.forEach((k) => (k.tag === "tr" ? (rows.length || (headed = k.kids.some((c) => c.tag === "th")), rows.push(k.kids.filter((c) => c.tag === "td" || c.tag === "th").map((c) => cell(inline(c.kids))))) : walk(k)));
  walk(n);
  if (!rows.length) return "";
  if (!headed) rows.unshift(rows[0].map(() => ""));
  const w = Math.max(...rows.map((r) => r.length));
  const pad = (r) => [...r, ...Array(w - r.length).fill("")];
  const line = (r) => "| " + pad(r).join(" | ") + " |";
  return [line(rows[0]), line(Array(w).fill("---")), ...rows.slice(1).map(line)].join("\n");
}

function block(n, depth) {
  const c = cls(n);
  switch (n.tag) {
    case "p": return inline(n.kids).trim();
    case "h1": case "h2": case "h3": case "h4": return "#".repeat(+n.tag[1]) + " " + inline(n.kids).trim();
    case "ul": case "ol": return list(n, depth);
    case "table": return table(n);
    case "hr": return "---";
    case "pre": {
      if (c.includes("frame")) return "";
      const t = text(n).replace(/\n$/, "");
      return "```\n" + t + "\n```";
    }
    case "figure": {
      const im = (function find(x) { for (const k of x.kids || []) { if (k.tag === "img") return k; const f = find(k); if (f) return f; } })(n);
      const cap2 = n.kids.find((k) => k.tag === "figcaption");
      return [im ? img(im) : "", cap2 ? `*${inline(cap2.kids).trim()}*` : ""].filter(Boolean).join("\n\n");
    }
    case "blockquote": return quote(blocks(n.kids, depth));
    case "div": {
      if (c.includes("window") || c.includes("screen") || c.includes("chrome")) return "";
      if (c.includes("wiki-cards")) return cards(n);
      if (c.includes("swatches")) return swatches(n);
      if (c.includes("theme-sample")) return "";
      const inner = blocks(n.kids, depth);
      return c.includes("note") ? quote(inner) : inner;
    }
  }
  return "";
}
const quote = (s) => s.split("\n").map((l) => "> " + l).join("\n");

function cards(n) {
  return n.kids
    .filter((a) => a.tag === "a")
    .map((a) => {
      const title = a.kids.find((k) => cls(k).includes("ctitle"));
      const desc = a.kids.find((k) => k.tag === "p");
      return `- [${text(title).trim()}](${link(a.attrs.href)})${desc ? ": " + inline(desc.kids).trim() : ""}`;
    })
    .join("\n");
}

function swatches(n) {
  return n.kids
    .filter((s) => cls(s).includes("swatch"))
    .map((s) => {
      const code = s.kids.find((k) => k.tag === "code");
      const label = s.kids.filter((k) => k.tag === "#text").map(text).join("").trim();
      return `\`${text(code)}\` ${label}`;
    })
    .join(" · ");
}

// ---------------------------------------------------------------- pages

const read = (f) => readFileSync(join(ROOT, "docs/wiki", f), "utf8");
const body = (html) => html.slice(html.indexOf("<!-- BODY -->") + 13, html.indexOf("<!-- /BODY -->"));

// First pass: heading ids -> GitHub slugs, so links to "#steps" keep working.
const parsed = new Map();
for (const p of PAGES) {
  const tree = parse(body(read(p.file)));
  parsed.set(p.file, tree);
  const m = new Map();
  (function walk(x) {
    for (const k of x.kids || []) {
      if (/^h[2-4]$/.test(k.tag) && k.attrs.id) m.set(k.attrs.id, slug(inline(k.kids).replace(/<[^>]+>/g, "").replace(/[`*]/g, "").trim()));
      walk(k);
    }
  })(tree);
  anchors.set(pageName(p.file), m);
}

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

for (const p of PAGES) {
  ctxFile = p.file;
  const md = blocks(parsed.get(p.file).kids);
  const url = SITE + "wiki/" + p.file;
  const text2 = [
    `# ${p.title}`,
    `> ${p.lede}`,
    md,
    "---",
    `*This page is mirrored from [the canopy website](${url}), where the screenshots and terminal screens live. To change it, open a pull request against \`docs/wiki\` in the repository.*`,
  ].join("\n\n");
  writeFileSync(join(out, pageName(p.file) + ".md"), text2.replace(/\n{3,}/g, "\n\n") + "\n");
}

// Sidebar and footer.
const side = ["**[Home](Home)**", "", "**Basics**", `- [${byFile.get("git-basics.html").title}](Git-basics)`, `- [Themes](Themes)`, ""];
for (const g of GUIDES) {
  side.push(`**[${g.name}](${pageName(`${g.dir}/index.html`)})**`);
  for (const [group, pages] of g.groups) {
    side.push("", `*${group}*`);
    for (const [f, title] of pages) side.push(`- [${title}](${pageName(`${g.dir}/${f}`)})`);
  }
  side.push("");
}
writeFileSync(join(out, "_Sidebar.md"), side.join("\n"));
writeFileSync(
  join(out, "_Footer.md"),
  `[Website](${SITE}) · [Download](${SITE}downloads.html) · [Changelog](${SITE}changelog.html) · [Contributing](https://github.com/shankar-sachin/canopy/blob/main/CONTRIBUTING.md) · [Code of conduct](https://github.com/shankar-sachin/canopy/blob/main/CODE_OF_CONDUCT.md)\n`,
);
console.log(`wiki-to-github: ${PAGES.length} pages -> ${out}`);
