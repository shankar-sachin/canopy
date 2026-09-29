#!/usr/bin/env node
// Audit the website in docs/ without a browser or any npm packages:
//
//   node scripts/audit-site.mjs          # errors fail (exit 1), warnings don't
//   node scripts/audit-site.mjs --strict # warnings fail too
//
// Checks every page for: doctype, lang, charset, viewport, a title and a
// description, exactly one <h1>, unique ids, alt text on images; that every
// local link, image, script and stylesheet points at a file that exists (and
// every #anchor at an id on that page), and every aria-controls or label
// `for` at an element; that nothing loads over plain http;
// that external scripts and styles only come from Google Fonts; that the
// search index lists every page with headings that exist; and that the
// newest version on the site is the one in Cargo.toml.
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, normalize, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { LEGACY, PAGES, RECIPE_SECTIONS, SHAPED_FILES, SHAPED_GROUPS, guidePages } from "./wiki-pages.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DOCS = join(ROOT, "docs");
const strict = process.argv.includes("--strict");

const errors = [];
const warnings = [];
const err = (file, msg) => errors.push(`${relative(ROOT, file)}: ${msg}`);
const warn = (file, msg) => warnings.push(`${relative(ROOT, file)}: ${msg}`);

function walk(dir) {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

const pages = walk(DOCS).filter((p) => p.endsWith(".html"));
const html = new Map(pages.map((p) => [p, readFileSync(p, "utf8")]));

// Generated terminal screenshots are big <pre> blocks of spans; they have no
// links or ids that matter, so leave them out of the scan.
const strip = (s) => s.replace(/<!-- SCREEN:(\w+) -->[\s\S]*?<!-- \/SCREEN:\1 -->/g, "").replace(/<!--[\s\S]*?-->/g, "");

const idsOf = new Map();
function ids(page) {
  if (!idsOf.has(page)) idsOf.set(page, [...html.get(page).matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]));
  return idsOf.get(page);
}

const attr = (tag, name) => tag.match(new RegExp(`\\s${name}="([^"]*)"`, "i"))?.[1];

for (const [page, raw] of html) {
  const s = strip(raw);
  if (!/^<!doctype html>/i.test(raw.trimStart())) err(page, "missing <!doctype html>");
  if (!/<html[^>]*\slang="[a-z-]+"/i.test(s)) err(page, "<html> has no lang");
  if (!/<meta charset="utf-8">/i.test(s)) err(page, "missing <meta charset=\"utf-8\">");
  if (!/<meta name="viewport" content="width=device-width, initial-scale=1">/.test(s)) err(page, "missing the viewport meta tag");
  const title = s.match(/<title>([^<]*)<\/title>/)?.[1]?.trim();
  if (!title) err(page, "no <title>");
  const desc = s.match(/<meta name="description" content="([^"]*)"/)?.[1];
  if (!desc) err(page, "no meta description");
  else if (desc.length > 200) warn(page, `meta description is ${desc.length} characters (search engines show about 160)`);
  const h1s = (s.match(/<h1[\s>]/g) || []).length;
  if (h1s !== 1) err(page, `has ${h1s} <h1> elements (want 1)`);

  const seen = new Set();
  for (const id of ids(page)) {
    if (seen.has(id)) err(page, `duplicate id="${id}"`);
    seen.add(id);
  }

  for (const [tag] of s.matchAll(/<img\b[^>]*>/g)) {
    if (attr(tag, "alt") === undefined) err(page, `image without alt: ${tag.slice(0, 80)}`);
    if (!attr(tag, "width") || !attr(tag, "height")) warn(page, `image without width/height (layout shift): ${attr(tag, "src")}`);
  }

  // External resources: fonts from Google only; nothing over plain http.
  for (const [tag] of s.matchAll(/<(script|link)\b[^>]*>/g)) {
    const url = attr(tag, "src") || attr(tag, "href");
    if (!url || !/^https?:/.test(url)) continue;
    const rel = attr(tag, "rel") || "";
    if (/^<link/.test(tag) && !/stylesheet|preconnect|icon/.test(rel)) continue;
    if (!/^https:\/\/fonts\.(googleapis|gstatic)\.com(\/|$)/.test(url)) err(page, `loads ${url} from outside (only Google Fonts is allowed)`);
  }
  for (const [, url] of s.matchAll(/\s(?:href|src)="(http:\/\/[^"]+)"/g)) {
    if (!/^http:\/\/(localhost|127\.0\.0\.1)/.test(url)) err(page, `plain-http link ${url}`);
  }

  // Local links and assets.
  for (const [, url] of s.matchAll(/\s(?:href|src)="([^"]*)"/g)) {
    if (/^(https?:|mailto:|data:|javascript:|\{)/.test(url) || url === "") continue;
    if (url.startsWith("//")) { err(page, `protocol-relative URL ${url}`); continue; }
    const [pathPart, hash] = url.split("#");
    const target = pathPart ? normalize(join(dirname(page), decodeURI(pathPart.split("?")[0]))) : page;
    if (!target.startsWith(DOCS)) { err(page, `link leaves the site: ${url}`); continue; }
    const file = existsSync(target) && statSync(target).isDirectory() ? join(target, "index.html") : target;
    if (!existsSync(file)) { err(page, `broken link ${url}`); continue; }
    if (hash && file.endsWith(".html") && !ids(file).includes(hash) && !["top", "main"].includes(hash)) {
      err(page, `link ${url}: no id="${hash}" on ${relative(DOCS, file)}`);
    }
  }

  // aria-controls and label-for must name an element on the page.
  for (const [, id] of s.matchAll(/\s(?:aria-controls|for)="([^"]+)"/g)) {
    if (!ids(page).includes(id)) err(page, `aria-controls/for="${id}" names no element`);
  }

  // Every external link that opens a new tab also cuts the opener.
  for (const [tag] of s.matchAll(/<a\b[^>]*target="_blank"[^>]*>/g)) {
    if (!/rel="[^"]*noopener/.test(tag)) warn(page, `target=_blank without rel="noopener": ${attr(tag, "href")}`);
  }
}

// Search index: every page is findable, and every heading it lists exists.
const indexFile = join(DOCS, "assets", "search-index.js");
if (existsSync(indexFile)) {
  const src = readFileSync(indexFile, "utf8");
  const json = src.slice(src.indexOf("["), src.lastIndexOf("]") + 1);
  let index = [];
  try { index = JSON.parse(json); } catch (e) { err(indexFile, `isn't valid JSON: ${e.message}`); }
  const listed = new Set();
  for (const entry of index) {
    const file = join(DOCS, entry.url);
    listed.add(file);
    if (!html.has(file)) { err(indexFile, `lists ${entry.url}, which doesn't exist`); continue; }
    for (const h of entry.headings || []) {
      if (!ids(file).includes(h.id)) err(indexFile, `${entry.url}#${h.id} ("${h.text}") isn't a heading on that page`);
    }
    if (!entry.title) err(indexFile, `${entry.url} has no title in the index`);
  }
  for (const page of pages) {
    if (!listed.has(page) && !html.get(page).includes("moved · canopy wiki")) warn(indexFile, `${relative(DOCS, page)} isn't in the search index`);
  }
}

// The wiki: every guide and recipe has the standard sections, long pages have
// headings, the previous/next links follow each guide's reading order, and the
// generated pages and search index are up to date.
{
  const wikiDir = join(DOCS, "wiki");
  const shaped = (p) => p.guide && (SHAPED_GROUPS.includes(p.group) || SHAPED_FILES.includes(p.file.split("/").pop()));
  for (const p of PAGES) {
    const file = join(wikiDir, p.file);
    if (!html.has(file)) { err(file, "is in scripts/wiki-pages.mjs but doesn't exist"); continue; }
    const s = strip(html.get(file));
    const body = s.match(/<article[\s\S]*<\/article>/)?.[0] ?? s;
    const h2s = [...body.matchAll(/<h2 id="([^"]+)"/g)].map((m) => m[1]);
    if (shaped(p)) {
      const at = RECIPE_SECTIONS.map((id) => h2s.indexOf(id));
      for (const [k, id] of RECIPE_SECTIONS.entries()) if (at[k] < 0) err(file, `missing the "${id}" section (<h2 id="${id}">)`);
      if (!at.includes(-1) && at.some((v, k) => k && v < at[k - 1])) err(file, `sections ${RECIPE_SECTIONS.join(", ")} are out of order`);
    }
    const words = body.replace(/<[^>]+>/g, " ").split(/\s+/).filter(Boolean).length;
    if (words > 400 && h2s.length < 2) err(file, `${words} words but only ${h2s.length} h2 headings`);
    if (/TODO/.test(body)) err(file, "still has a TODO");
    if (p.guide) {
      const order = guidePages(p.guide);
      const i = order.indexOf(p);
      const pager = s.match(/<nav class="pager"[\s\S]*?<\/nav>/)?.[0] ?? "";
      const links = [...pager.matchAll(/href="([^"]+)"/g)].map((m) => normalize(join(dirname(file), m[1])));
      const want = [order[i - 1], order[i + 1]].filter(Boolean).map((q) => join(wikiDir, q.file));
      if (links.join() !== want.join()) err(file, `previous/next links are ${links.map((l) => relative(wikiDir, l)).join(", ") || "missing"}, want ${want.map((l) => relative(wikiDir, l)).join(", ")}`);
    }
    // canopy console pages never mention the desktop app's buttons as keys, and vice versa.
    if (p.guide === "desktop" && /<kbd>(?!<)/.test(body)) err(file, "has a plain <kbd> (desktop shortcuts are <kbd class=\"d\">)");
    if (p.guide === "console" && /<kbd class="d">/.test(body)) err(file, "has a desktop <kbd class=\"d\"> shortcut in a console page");
    if (/canopy for the terminal|terminal app|\bthe TUI\b/i.test(body.replace(/<[^>]+>/g, " "))) err(file, "says \"canopy for the terminal\" / \"terminal app\" (it's \"canopy console\" now)");
  }
  const known = new Set([...PAGES.map((p) => p.file), ...Object.keys(LEGACY)]);
  for (const page of pages) {
    if (page.startsWith(wikiDir) && !known.has(relative(wikiDir, page))) err(page, "isn't in scripts/wiki-pages.mjs (no sidebar, pager or search entry)");
  }
  const r = spawnSync(process.execPath, [join(ROOT, "scripts", "wiki.mjs"), "--check"], { encoding: "utf8" });
  if (r.status !== 0) err(wikiDir, `node scripts/wiki.mjs --check failed: ${(r.stdout + r.stderr).trim().split("\n")[0]}`);
}

// The brand is lowercase in prose ("canopy", not "Canopy").
for (const [page, raw] of html) {
  const text = strip(raw).replace(/<(script|style)[\s\S]*?<\/\1>/g, "").replace(/<[^>]+>/g, " ");
  const m = text.match(/.{0,30}\bCanopy\b.{0,20}/);
  if (m) err(page, `capital-C "Canopy": "${m[0].trim()}"`);
  const old = text.match(/.{0,30}(canopy for the terminal|canopy terminal|terminal app).{0,20}/i);
  if (old && !page.endsWith("changelog.html")) err(page, `old name for canopy console: "${old[0].trim()}"`);
}

// The newest release on the site matches the version being built.
const version = readFileSync(join(ROOT, "Cargo.toml"), "utf8").match(/^version = "([^"]+)"/m)?.[1];
const changelog = join(DOCS, "changelog.html");
if (version && html.has(changelog)) {
  const newest = html.get(changelog).match(/<div class="release" id="v[\d-]+"><h2>v(\d+\.\d+\.\d+)/)?.[1];
  if (newest !== version) err(changelog, `newest version is ${newest}, but Cargo.toml says ${version} (add a changelog entry)`);
}

for (const w of warnings) console.log(`warning: ${w}`);
for (const e of errors) console.log(`error: ${e}`);
const failed = errors.length || (strict && warnings.length);
console.log(`audit-site: ${pages.length} pages, ${errors.length} errors, ${warnings.length} warnings`);
process.exit(failed ? 1 : 0);
