#!/usr/bin/env node
// The third-party license notices that ship with canopy, and a license check.
//
//   node scripts/third-party.mjs           # rewrite THIRD-PARTY-LICENSES.txt
//   node scripts/third-party.mjs --check   # change nothing; fail if it is stale
//                                          # or a dependency has a license we don't allow
//
// It walks the real dependency graph (`cargo metadata`, normal dependencies of
// every workspace crate, only the features that are switched on), so it lists
// what is compiled into the apps and nothing else. Each package's own license
// files are copied in, so the "keep this notice with the software" terms of
// MIT, Apache-2.0, BSD, ISC, Zlib and Unicode are met. MPL-2.0 packages are
// listed with where to get their source.
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(ROOT, "THIRD-PARTY-LICENSES.txt");
const check = process.argv.includes("--check");

// A dependency is fine if at least one way of licensing it (an "OR" branch)
// uses only these. Anything else fails, so a GPL or AGPL crate can't slip in.
const ALLOWED = new Set(["MIT", "MIT-0", "Apache-2.0", "LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "Unicode-3.0", "Unicode-DFS-2016", "0BSD", "Unlicense", "CC0-1.0", "BSL-1.0", "MPL-2.0"]);
const COPYLEFT = new Set(["MPL-2.0"]);

const r = spawnSync("cargo", ["metadata", "--format-version", "1", "--locked"], { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
if (r.status !== 0) {
  console.error(r.stderr);
  process.exit(1);
}
const meta = JSON.parse(r.stdout);

// What is really compiled in: `cargo tree` follows the enabled features and
// leaves out optional crates that stay off. (`cargo metadata` lists those too.)
const t = spawnSync("cargo", ["tree", "--workspace", "--target", "all", "-e", "normal", "--prefix", "none", "--format", "{p}", "--locked"], { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
if (t.status !== 0) {
  console.error(t.stderr);
  process.exit(1);
}
const compiled = new Set(t.stdout.split("\n").map((l) => l.match(/^(\S+) v(\S+)/)).filter(Boolean).map((m) => `${m[1]} ${m[2]}`));
const seen = new Set(meta.packages.filter((p) => compiled.has(`${p.name} ${p.version}`)).map((p) => p.id));
const members = new Set(meta.workspace_members);
const pkgs = meta.packages.filter((p) => seen.has(p.id) && !members.has(p.id))
  .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));

const branches = (expr) => expr.replace(/\//g, " OR ").replace(/[()]/g, " ").split(/\s+OR\s+/i).map((b) => b.split(/\s+(?:AND|WITH)\s+/i).map((t) => t.trim()).filter(Boolean));
const problems = [];
const info = pkgs.map((p) => {
  const expr = p.license || "";
  if (!expr) problems.push(`${p.name} ${p.version} declares no license (license_file: ${p.license_file ?? "none"})`);
  else if (!branches(expr).some((b) => b.every((t) => ALLOWED.has(t)))) problems.push(`${p.name} ${p.version}: license "${expr}" isn't allowed (see ALLOWED in scripts/third-party.mjs)`);
  const dir = dirname(p.manifest_path);
  const files = readdirSync(dir).filter((f) => /^(licen[cs]e|copying|notice|unlicense)/i.test(f)).sort();
  const texts = files.map((f) => readFileSync(join(dir, f), "utf8").replace(/\r\n/g, "\n").trim()).filter(Boolean);
  return { p, expr, texts, mpl: branches(expr).every((b) => b.some((t) => COPYLEFT.has(t))) };
});

const key = (t) => createHash("sha256").update(t).digest("hex").slice(0, 16);
const uniq = new Map();
for (const { p, texts } of info) {
  for (const t of texts) {
    const k = key(t);
    if (!uniq.has(k)) uniq.set(k, { text: t, users: [] });
    uniq.get(k).users.push(`${p.name} ${p.version}`);
  }
}
// Some crates publish without their license file. For those we print the
// license they declare, with the authors from Cargo.toml as the copyright holders.
const findText = (re) => [...uniq.values()].find((u) => re.test(u.text))?.text;
const canon = {
  "Apache-2.0": findText(/Apache License\s+Version 2\.0/),
  "MPL-2.0": findText(/Mozilla Public License,? Version 2\.0|Mozilla Public License Version 2\.0/),
  "BSD-3-Clause": (who) => `BSD 3-Clause License\n\nCopyright (c) ${who}\nAll rights reserved.\n\nRedistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:\n\n1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.\n\n2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.\n\n3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.\n\nTHIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.`,
  MIT: (who) => `MIT License\n\nCopyright (c) ${who}\n\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.`,
};
const fallbacks = [];
for (const it of info) {
  if (it.texts.length) continue;
  const { p } = it;
  const ok = branches(p.license).filter((b) => b.every((t) => ALLOWED.has(t))).flat();
  const id = ["MIT", "BSD-3-Clause", "Apache-2.0", "MPL-2.0"].find((i) => ok.includes(i));
  if (!id || !canon[id]) {
    problems.push(`${p.name} ${p.version} has no license file and no standard text to fall back on (declared ${p.license})`);
    continue;
  }
  const who = p.authors.map((a) => a.replace(/\s*<[^>]*>/, "")).join(", ") || `the ${p.name} authors`;
  fallbacks.push({ p, id, who });
}

const rule = "=".repeat(78);
let out = `canopy: third-party software
${rule}

canopy itself is MIT licensed (see LICENSE). It is built from the open source
packages below, each under its own license. This file lists them and repeats
their license texts, as those licenses ask. It is generated by
scripts/third-party.mjs from the dependency graph, so it matches what is
compiled into canopy console and canopy desktop.

${pkgs.length} packages.

Packages under the Mozilla Public License 2.0 (MPL-2.0) are used unmodified.
Their source is available from the addresses listed for them below, and from
https://crates.io/crates/<name>.

${rule}
Bundled data that isn't a Rust package
${rule}

Syntax definitions (the syntect crate's bundled set, used to color code in diffs)
    Taken from the Sublime Text "Packages" repository,
    https://github.com/sublimehq/Packages, by way of the syntect crate
    (https://github.com/trishume/syntect). They are distributed under the
    permissive licenses stated in those repositories.

Brand icons in canopy desktop (the Claude, Codex and GitHub marks)
    From Simple Icons, https://simpleicons.org, released under CC0-1.0.
    The marks themselves are trademarks of their owners and are shown only to
    name the service they stand for.

${rule}
Packages
${rule}

`;
for (const { p, expr, mpl } of info) {
  out += `${p.name} ${p.version}\n    license: ${expr || "(none declared)"}${mpl ? "   [MPL-2.0: source at " + (p.repository || `https://crates.io/crates/${p.name}`) + "]" : ""}\n    ${p.repository || p.homepage || `https://crates.io/crates/${p.name}`}\n`;
}
out += `\n${rule}\nLicense texts\n${rule}\n`;
const sorted = [...uniq.values()].sort((a, b) => b.users.length - a.users.length || a.users[0].localeCompare(b.users[0]));
for (const { text, users } of sorted) {
  out += `\n${"-".repeat(78)}\nUsed by: ${users.join(", ")}\n${"-".repeat(78)}\n\n${text}\n`;
}

if (fallbacks.length) {
  out += `\n${rule}\nPackages that publish no license file\n${rule}\n\nThese packages declare the license shown in their Cargo.toml but do not include\nthe text. It is repeated here, with the authors they list.\n`;
  for (const id of ["MIT", "BSD-3-Clause", "Apache-2.0", "MPL-2.0"]) {
    const group = fallbacks.filter((f) => f.id === id);
    if (!group.length) continue;
    out += `\n${"-".repeat(78)}\n${id}: ${group.map(({ p }) => `${p.name} ${p.version}`).join(", ")}\n${"-".repeat(78)}\n\n`;
    if (typeof canon[id] === "function") out += group.map(({ p, who }) => `${p.name}: ${canon[id](who)}`).join("\n\n") + "\n";
    else out += `${canon[id]}\n`;
  }
}

if (problems.length) {
  for (const p of problems) console.error(`third-party: ${p}`);
  process.exit(1);
}
if (check) {
  const now = existsSync(OUT) ? readFileSync(OUT, "utf8") : "";
  if (now !== out) {
    console.error("THIRD-PARTY-LICENSES.txt is out of date: run `node scripts/third-party.mjs`");
    process.exit(1);
  }
  console.log(`third-party: ${pkgs.length} packages, licenses allowed, THIRD-PARTY-LICENSES.txt up to date`);
} else {
  writeFileSync(OUT, out);
  console.log(`wrote THIRD-PARTY-LICENSES.txt: ${pkgs.length} packages, ${uniq.size} distinct license texts, ${(out.length / 1024).toFixed(0)} KB`);
}
