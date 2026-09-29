#!/usr/bin/env node
// Builds the wiki's frame and the site's search index, so pages only hold
// their own words:
//
//   node scripts/wiki.mjs           # rewrite docs/wiki/**/*.html and docs/assets/search-index.js
//   node scripts/wiki.mjs --check   # change nothing; fail if either is out of date
//
// The wiki has a home, pages both apps share, and two guides: canopy desktop
// (docs/wiki/desktop/) and canopy console (docs/wiki/console/). The page list
// is in scripts/wiki-pages.mjs. Each page keeps its content between
// <!-- BODY --> and <!-- /BODY -->; everything around it comes from here: the
// <head>, the site header (copied from docs/wiki/index.html), the sidebar,
// breadcrumbs, title and lede, and the Previous / Next links, which follow
// each guide's order. Guide homes list their pages as cards. Old flat URLs
// (docs/wiki/undo.html) become small "this page moved" pages.
// To add a page: add it to scripts/wiki-pages.mjs, create the file with just
// the two markers and its content, and run this script.
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { GUIDES, LEGACY, PAGES, RENAMED, guidePages } from "./wiki-pages.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DOCS = join(ROOT, "docs");
const WIKI = join(DOCS, "wiki");
const check = process.argv.includes("--check");

// Site pages outside the wiki, for the search index: [title, section].
const SITE = {
  "index.html": ["canopy", "Home"],
  "desktop.html": ["canopy desktop", "Product"],
  "features.html": ["canopy console", "Product"],
  "github.html": ["GitHub", "Product"],
  "changelog.html": ["Changelog", "Product"],
  "download.html": ["Downloads", "Docs"],
  "get-started.html": ["Get started", "Docs"],
  "keys.html": ["Console keys", "Docs"],
};

const problems = [];
const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const read = (p) => readFileSync(p, "utf8");
const CHEV = `<svg class="chev" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m9 6 6 6-6 6"/></svg>`;
const depth = (file) => file.split("/").length - 1;
const up = (file) => "../".repeat(depth(file) + 1); // from the page to docs/

// ------------------------------------------------------------- the frame

// The header and stylesheet links are copied from the wiki home, written for a
// page one folder below docs/; other depths get their "../" rewritten.
const home = read(join(WIKI, "index.html"));
const between = (s, a, b) => {
  const i = s.indexOf(a);
  const j = s.indexOf(b, i + a.length);
  return i < 0 || j < 0 ? null : s.slice(i, j + b.length);
};
const HEADER_HOME = between(home, '<header class="topbar"', "</header>");
// The header without any "you are here" marks; header(file) adds this page's.
const HEADER = HEADER_HOME && HEADER_HOME.replace(/ aria-current="page"/g, "").replace(/<div class="menu active"/g, '<div class="menu"');
const STYLES = between(home, '<link rel="preconnect"', 'wiki.css">');
if (!HEADER || !STYLES) throw new Error("docs/wiki/index.html has no site header or stylesheet links to copy");
const rebase = (html, file) => html.replace(/((?:href|src|data-root)=")\.\.\//g, `$1${up(file)}`);

function header(file) {
  const wiki = `<a href="../wiki/${file}"`;
  let h = HEADER;
  let active = h.indexOf(wiki) >= 0 ? wiki : `<a href="../wiki/index.html"`;
  h = h.replace(active, active + ' aria-current="page"');
  const parts = h.split('<div class="menu"');
  h = parts.map((part, i) => (i > 0 && part.includes('aria-current="page"') ? '<div class="menu active"' : i > 0 ? '<div class="menu"' : "") + part).join("");
  return rebase(h, file);
}

// Headings are written as <h2 id="x">Title</h2>; the # link is added here.
const withHashLinks = (html) => html.replace(/<h([23]) id="([^"]+)">((?:(?!<a class="hash")[\s\S])*?)<\/h\1>/g,
  (_, n, id, inner) => `<h${n} id="${id}">${inner}<a class="hash" href="#${id}" aria-label="Link to this section">#</a></h${n}>`);

function body(file) {
  const path = join(WIKI, file);
  if (!existsSync(path)) {
    problems.push(`docs/wiki/${file} doesn't exist (it's in scripts/wiki-pages.mjs)`);
    return "";
  }
  const s = read(path);
  const a = s.indexOf("<!-- BODY -->");
  const b = s.indexOf("<!-- /BODY -->");
  if (a < 0 || b < a) {
    problems.push(`docs/wiki/${file} has no <!-- BODY --> markers`);
    return "";
  }
  return withHashLinks(s.slice(a + "<!-- BODY -->".length, b).replace(/^\n/, "").replace(/\s+$/, ""));
}

const linkTo = (from, to) => {
  // A relative link from one wiki page to another.
  const a = dirname(from) === "." ? [] : dirname(from).split("/");
  const b = dirname(to) === "." ? [] : dirname(to).split("/");
  while (a.length && b.length && a[0] === b[0]) { a.shift(); b.shift(); }
  return [...a.map(() => ".."), ...b, to.split("/").pop()].join("/");
};

function sidebar(current) {
  const tree = (g, open) => g.groups.map(([group, pages]) => {
    const isOpen = open && (group !== "Reference" || pages.some(([f]) => `${g.dir}/${f}` === current.file));
    const links = pages.map(([f, title]) => {
      const file = `${g.dir}/${f}`;
      return `<a href="${linkTo(current.file, file)}"${file === current.file ? ' aria-current="page"' : ""}>${esc(title)}</a>`;
    }).join("\n");
    return `<details${isOpen ? " open" : ""}><summary>${group}${CHEV}</summary><div class="wgroup">\n${links}\n</div></details>`;
  }).join("\n");
  const switcher = `<div class="wswitch" role="group" aria-label="Choose an app">${GUIDES.map((g) =>
    `<a href="${linkTo(current.file, `${g.dir}/index.html`)}"${current.guide === g.id ? ' aria-current="true"' : ""}>${esc(g.name)}</a>`).join("")}</div>`;
  const shared = PAGES.filter((p) => !p.guide && p.file !== "index.html");
  const both = `<details open><summary>Both apps${CHEV}</summary><div class="wgroup">\n<a href="${linkTo(current.file, "index.html")}"${current.file === "index.html" ? ' aria-current="page"' : ""}>Wiki home</a>\n${shared.map((p) => `<a href="${linkTo(current.file, p.file)}"${p.file === current.file ? ' aria-current="page"' : ""}>${esc(p.title)}</a>`).join("\n")}\n</div></details>`;
  const g = GUIDES.find((x) => x.id === current.guide);
  const extra = g && g.id === "console" ? `<details open><summary>Elsewhere${CHEV}</summary><div class="wgroup">\n<a href="${up(current.file)}features.html">Feature tour</a>\n</div></details>` : "";
  return `${switcher}\n${both}\n${g ? tree(g, true) : ""}${extra ? `\n${extra}` : ""}`;
}

function pager(p) {
  if (!p.guide) return "";
  const list = guidePages(p.guide);
  const i = list.indexOf(p);
  const prev = list[i - 1];
  const next = list[i + 1];
  const a = (q, cls, label) => `<a href="${linkTo(p.file, q.file)}"${cls}><small>${label}</small>${esc(q.title)}</a>`;
  return `<nav class="pager" aria-label="Pages">${prev ? a(prev, "", "Previous") : "<span></span>"}${next ? a(next, ' class="next"', "Next") : "<span></span>"}</nav>`;
}

const hashed = (id, text) => `<h2 id="${id}">${text}<a class="hash" href="#${id}" aria-label="Link to this section">#</a></h2>`;
const cardsHtml = (from, list) => `<div class="wiki-cards">${list.map((p) => `<a class="card" href="${linkTo(from, p.file)}"><span class="ctitle">${esc(p.title)}</span><p>${esc(p.card)}</p></a>`).join("")}</div>`;

// Home pages: their own words, then cards for the pages below them.
function homeBody(p) {
  const own = body(p.file).replace(/\n?<!-- CARDS[\s\S]*$/, "");
  let cards;
  if (p.guide) {
    const g = GUIDES.find((x) => x.id === p.guide);
    cards = g.groups.map(([group, pages]) => {
      const id = group.toLowerCase().replace(/\s+/g, "-");
      const list = pages.map(([f]) => PAGES.find((x) => x.file === `${g.dir}/${f}`));
      return `${hashed(id, group)}\n${cardsHtml(p.file, list)}`;
    }).join("\n");
  } else {
    const shared = PAGES.filter((x) => !x.guide && x.file !== "index.html");
    cards = `${hashed("both", "For both apps")}\n${cardsHtml(p.file, shared)}\n` + GUIDES.map((g) =>
      `${hashed(g.id, esc(g.name))}\n<p>${esc(g.blurb)}</p>\n${cardsHtml(p.file, guidePages(g.id).filter((x) => ["Guides", "Recipes"].includes(x.group)).slice(0, 0).concat(PAGES.find((x) => x.file === `${g.dir}/index.html`), ...g.groups[0][1].map(([f]) => PAGES.find((x) => x.file === `${g.dir}/${f}`))))}`).join("\n");
  }
  return `${own}\n<!-- CARDS: generated by scripts/wiki.mjs from here to the end -->\n${cards}`;
}

function page(p) {
  const title = `${p.title} · ${p.guide ? p.name : "canopy"} wiki`;
  const content = p.file === "index.html" || p.file.endsWith("/index.html") ? homeBody(p) : body(p.file);
  const u = up(p.file);
  const g = GUIDES.find((x) => x.id === p.guide);
  const other = g && GUIDES.find((x) => x.id !== g.id);
  const banner = !g
    ? `<b>New to git?</b> Start with <a href="${u}desktop.html">canopy desktop</a>, the friendly app for macOS, Windows and Linux. Spot something wrong or unclear? <a href="https://github.com/shankar-sachin/canopy/edit/main/docs/wiki/${p.file}">Suggest an edit</a>.`
    : g.id === "desktop"
      ? `This is the <b>canopy desktop</b> guide. Prefer the keyboard? <a href="${linkTo(p.file, "console/index.html")}">Switch to the canopy console guide</a>. Spot something wrong? <a href="https://github.com/shankar-sachin/canopy/edit/main/docs/wiki/${p.file}">Suggest an edit</a>.`
      : `This is the <b>canopy console</b> guide. New to git? <a href="${linkTo(p.file, "desktop/index.html")}">Switch to the canopy desktop guide</a>. Spot something wrong? <a href="https://github.com/shankar-sachin/canopy/edit/main/docs/wiki/${p.file}">Suggest an edit</a>.`;
  const crumbs = g
    ? `<a href="${linkTo(p.file, "index.html")}">Wiki</a><span>/</span><a href="${linkTo(p.file, `${g.dir}/index.html`)}">${esc(g.name)}</a>${p.group === "Home" ? "" : `<span>/</span>${p.group}`}`
    : `<a href="${linkTo(p.file, "index.html")}">Wiki</a><span>/</span>${p.file === "index.html" ? "Home" : "Both apps"}`;
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)}</title>
<meta name="description" content="${esc(p.lede)}">
<meta property="og:title" content="${esc(title)}">
<meta property="og:image" content="https://shankar-sachin.github.io/canopy/assets/og.png">
<meta name="theme-color" content="#0b100c">
<script>try{const t=localStorage.getItem("canopy-theme");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t;}catch(e){}</script>
<link rel="icon" href="${u}assets/logo.svg" type="image/svg+xml">
${rebase(STYLES, p.file)}
</head>
<body class="wiki">
<a class="skip" href="#content">Skip to content</a>
${header(p.file)}
<div class="wlayout">
  <aside class="wside" id="wside"><nav class="wside-nav" aria-label="Wiki">
${sidebar(p)}
</nav></aside>
  <main class="wmain" id="content">
    <div class="wbanner">${banner}</div>
    <article class="wprose">
      <div class="crumbs">${crumbs}</div>
      <h1>${esc(p.title)}</h1>
      <p class="wlede">${esc(p.lede)}</p>
<!-- BODY -->
${content}
<!-- /BODY -->
${pager(p)}
    </article>
  </main>
  <aside class="wtoc" aria-label="On this page"><b>On this page</b><nav id="toc"></nav></aside>
</div>
<script src="${u}assets/site.js"></script>
<script src="${u}assets/wiki.js"></script>
</body>
</html>
`;
}

// An old flat URL: a page that says where its two versions went.
function moved(file, targets) {
  const u = "../";
  const links = targets.map((id) => {
    const g = GUIDES.find((x) => x.id === id);
    const to = RENAMED[file] ?? file;
    const p = PAGES.find((x) => x.file === `${g.dir}/${to}`);
    return { g, p, href: `${g.dir}/${to}` };
  });
  const first = links[0].href;
  const title = links[0].p?.title ?? file;
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)} moved · canopy wiki</title>
<meta name="description" content="This wiki page now has one version for each app.">
<meta name="robots" content="noindex">
<meta http-equiv="refresh" content="4; url=${first}">
<link rel="canonical" href="https://shankar-sachin.github.io/canopy/wiki/${first}">
<link rel="icon" href="${u}assets/logo.svg" type="image/svg+xml">
<link rel="stylesheet" href="${u}assets/site.css">
</head>
<body>
<main class="wrap" style="max-width:640px;padding:96px 20px">
<h1>${esc(title)} moved</h1>
<p>The wiki now has a guide for each app. Pick yours:</p>
<ul>${links.map((l) => `<li><a href="${l.href}">${esc(l.p?.title ?? file)} in ${esc(l.g.name)}</a></li>`).join("")}</ul>
<p><a href="index.html">Wiki home</a></p>
</main>
</body>
</html>
`;
}

const outputs = new Map();
for (const p of PAGES) outputs.set(join(WIKI, p.file), page(p));
for (const [file, targets] of Object.entries(LEGACY)) outputs.set(join(WIKI, file), moved(file, targets));
const known = new Set([...PAGES.map((p) => p.file), ...Object.keys(LEGACY)]);
const walk = (d) => readdirSync(d).flatMap((n) => (statSync(join(d, n)).isDirectory() ? walk(join(d, n)) : [join(d, n)]));
for (const f of walk(WIKI).filter((f) => f.endsWith(".html")).map((f) => f.slice(WIKI.length + 1))) {
  if (!known.has(f)) problems.push(`docs/wiki/${f} isn't in scripts/wiki-pages.mjs`);
}

// ------------------------------------------------------------- search index

const text = (html) => html
  .replace(/<!-- SCREEN:(\w+) -->[\s\S]*?<!-- \/SCREEN:\1 -->/g, " ")
  .replace(/<(script|style|svg|pre class="frame")[\s\S]*?<\/(script|style|svg|pre)>/g, " ")
  .replace(/<[^>]+>/g, " ")
  .replace(/&nbsp;/g, " ").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&middot;/g, "·")
  .replace(/\s+/g, " ").trim();

function headings(html) {
  return [...html.matchAll(/<h([23]) id="([^"]+)"[^>]*>([\s\S]*?)<\/h\1>/g)].map((m) => ({
    id: m[2],
    text: text(m[3].replace(/<a class="hash"[\s\S]*?<\/a>/, "")),
  }));
}

const index = [];
for (const p of PAGES) {
  const html = outputs.get(join(WIKI, p.file));
  const article = between(html, "<!-- BODY -->", "<!-- /BODY -->") || "";
  index.push({ url: `wiki/${p.file}`, title: p.title, section: p.guide ? p.name : "Wiki", lede: p.lede, headings: headings(article), text: text(article).slice(0, 4000) });
}
for (const f of readdirSync(DOCS).filter((f) => f.endsWith(".html"))) {
  if (!SITE[f]) {
    problems.push(`docs/${f} has no entry in SITE in scripts/wiki.mjs (its search title and section)`);
    continue;
  }
  const html = read(join(DOCS, f))
    .replace(/<header class="topbar"[\s\S]*?<\/header>/, "")
    .replace(/<footer[\s\S]*?<\/footer>/, "")
    .replace(/<nav[\s\S]*?<\/nav>/g, "");
  const lede = read(join(DOCS, f)).match(/<meta name="description" content="([^"]*)"/)?.[1] || "";
  const [title, section] = SITE[f];
  const body = html.slice(html.indexOf("<body"));
  index.push({ url: f, title, section, lede: lede.replace(/&amp;/g, "&").replace(/&quot;/g, '"'), headings: headings(body), text: text(body).slice(0, f === "changelog.html" ? 8000 : 4000) });
}
outputs.set(join(DOCS, "assets", "search-index.js"),
  `// Generated by scripts/wiki.mjs: every page's title, headings and text, for the site search.\nwindow.CANOPY_SEARCH = ${JSON.stringify(index)};\n`);

// ------------------------------------------------------------- write / check

let stale = 0;
for (const [path, html] of outputs) {
  const now = existsSync(path) ? read(path) : "";
  if (now === html) continue;
  stale++;
  if (!check) mkdirSync(dirname(path), { recursive: true });
  if (check) problems.push(`${path.slice(ROOT.length + 1)} is out of date (run: node scripts/wiki.mjs)`);
  else writeFileSync(path, html);
}
for (const p of problems) console.log(`error: ${p}`);
console.log(`wiki: ${PAGES.length} pages, ${index.length} search entries, ${check ? `${stale} out of date` : `${stale} written`}`);
process.exit(problems.length ? 1 : 0);
