// Downloads page: every file of every release, grouped by app and system,
// with the visitor's system first. Reads the releases from GitHub (see
// canopyReleases in site.js), so a new release shows up with no site change.
"use strict";

(function () {
  const body = document.getElementById("dl-body");
  const pick = document.getElementById("dl-version");
  const meta = document.getElementById("dl-meta");
  if (!body || !pick) return;

  const REPO = "https://github.com/shankar-sachin/canopy";
  const RAW = "https://raw.githubusercontent.com/shankar-sachin/canopy/main/scripts";
  const OS_NAME = { mac: "macOS", windows: "Windows", linux: "Linux" };
  const here = visitorOS();
  const params = new URLSearchParams(location.search);
  let product = params.get("app") === "terminal" ? "terminal" : "desktop";
  let releases = [];

  // Which file is which: [product, system, label, detail] from its name.
  const KINDS = [
    [/^canopy-desktop-.*-aarch64-apple-darwin\.dmg$/, "desktop", "mac", "Apple silicon", ".dmg · M1 and later"],
    [/^canopy-desktop-.*-x86_64-apple-darwin\.dmg$/, "desktop", "mac", "Intel", ".dmg"],
    [/^canopy-desktop-.*-x86_64-setup\.exe$/, "desktop", "windows", "x64", "installer · most PCs"],
    [/^canopy-desktop-.*-aarch64-setup\.exe$/, "desktop", "windows", "Arm", "installer · Snapdragon and other Arm PCs"],
    [/^canopy-desktop-.*-amd64\.deb$/, "desktop", "linux", "Debian / Ubuntu", ".deb · x86_64"],
    [/^canopy-desktop-.*-x86_64\.AppImage$/, "desktop", "linux", "AppImage", "any distribution · x86_64"],
    [/^canopy-v.*-aarch64-apple-darwin\.tar\.gz$/, "terminal", "mac", "Apple silicon", ".tar.gz · M1 and later"],
    [/^canopy-v.*-x86_64-apple-darwin\.tar\.gz$/, "terminal", "mac", "Intel", ".tar.gz"],
    [/^canopy-v.*-x86_64-pc-windows-msvc\.zip$/, "terminal", "windows", "x64", ".zip · most PCs"],
    [/^canopy-v.*-aarch64-pc-windows-msvc\.zip$/, "terminal", "windows", "Arm", ".zip"],
    [/^canopy-v.*-x86_64-unknown-linux-gnu\.tar\.gz$/, "terminal", "linux", "x86_64", ".tar.gz"],
    [/^canopy-v.*-aarch64-unknown-linux-gnu\.tar\.gz$/, "terminal", "linux", "Arm64", ".tar.gz"],
  ];

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const mb = (n) => (n >= 1048576 ? (n / 1048576).toFixed(1) + " MB" : Math.max(1, Math.round(n / 1024)) + " KB");
  const day = (iso) => new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });

  let copyId = 0;
  function cmd(text, note) {
    const id = "dl-cmd-" + copyId++;
    return `<div class="dl-cmd"><div class="install"><span class="prompt">$</span><code id="${id}">${esc(text)}</code>
      <button class="copy" data-copy="${id}" type="button" aria-label="Copy" title="Copy"><svg class="i-copy" viewBox="0 0 16 16" width="15" height="15" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><rect x="5.5" y="5.5" width="8" height="8" rx="1.6"/><path d="M10.5 5.5V3.6A1.1 1.1 0 0 0 9.4 2.5H3.6a1.1 1.1 0 0 0-1.1 1.1v5.8a1.1 1.1 0 0 0 1.1 1.1h1.9"/></svg></button></div>
      ${note ? `<p class="dl-note">${note}</p>` : ""}</div>`;
  }

  /// Package-manager and script commands for a system. `latest` is whether
  /// the chosen version is the newest one (package managers install that).
  function commands(prod, os, rel, latest, files) {
    const v = rel.tag;
    const tool = os === "mac" ? "Homebrew installs" : "winget installs";
    const pinned = latest ? "" : `${tool} the latest version; for ${esc(v)}, download a file below.`;
    if (prod === "desktop") {
      if (os === "mac") return latest ? cmd("brew install --cask shankar-sachin/canopy/canopy-desktop", "With Homebrew. Update later with <code>brew upgrade --cask canopy-desktop</code>.") : `<p class="dl-note">${pinned}</p>`;
      if (os === "windows") return latest ? cmd("winget install shankars.canopy-desktop", "With winget. New versions can take a few days to reach winget; the installer below is always current.") : `<p class="dl-note">${pinned}</p>`;
      const deb = files.find((f) => f.name.endsWith(".deb"));
      return deb ? cmd(`curl -LO ${deb.url} && sudo apt install ./${deb.name}`, "Debian, Ubuntu and friends. Anywhere else, use the AppImage.") : "";
    }
    if (os === "mac") {
      const brew = latest ? cmd("brew install shankar-sachin/canopy/canopy", "With Homebrew (a prebuilt binary, nothing to compile).") : "";
      return brew + cmd(`curl -fsSL ${RAW}/install.sh | sh${latest ? "" : " -s -- " + v}`, "Or the install script: it checks the download and puts <code>canopy</code> in <code>~/.local/bin</code>.");
    }
    if (os === "linux") {
      return cmd(`curl -fsSL ${RAW}/install.sh | sh${latest ? "" : " -s -- " + v}`, "The install script: it checks the download and puts <code>canopy</code> in <code>~/.local/bin</code>, no sudo.");
    }
    const winget = latest ? cmd("winget install shankars.canopy", "With winget (it installs git too).") : "";
    const ps = latest
      ? `irm ${RAW}/install.ps1 | iex`
      : `& ([scriptblock]::Create((irm ${RAW}/install.ps1))) -Version ${v}`;
    return winget + cmd(ps, "Or in PowerShell, with the install script (no admin rights needed).");
  }

  function fileLinks(files, rel, os) {
    if (!files.length) {
      const first = product === "desktop" && os === "windows" ? " Canopy Desktop for Windows arrived in v1.0.3." : "";
      return `<p class="dl-note">No files for this system in ${esc(rel.tag)}.${first}</p>`;
    }
    return `<ul class="dl-files">${files
      .map((f) => {
        const sum = rel.assets.find((a) => a.name === f.name + ".sha256");
        return `<li><a class="dl-file" href="${esc(f.url)}"><b>${esc(f.label)}</b><span>${esc(f.detail)} · ${mb(f.size)}</span></a>
          ${sum ? `<a class="dl-sum" href="${esc(sum.url)}" title="SHA-256 checksum">sha256</a>` : ""}</li>`;
      })
      .join("")}</ul>`;
  }

  function render() {
    const rel = releases.find((r) => r.tag === pick.value) || releases[0];
    if (!rel) return;
    const latest = rel === releases.find((r) => !r.pre);
    meta.innerHTML = `${latest ? `<span class="dl-latest">Latest</span> ` : ""}Released ${day(rel.date)} · <a href="${esc(rel.url)}">Release notes →</a>`;
    const files = {};
    const known = new Set();
    for (const a of rel.assets) {
      const i = KINDS.findIndex(([re]) => re.test(a.name));
      if (i < 0) continue;
      const k = KINDS[i];
      known.add(a.name);
      const [, prod, os, label, detail] = k;
      if (prod !== product) continue;
      (files[os] = files[os] || []).push({ ...a, label, detail, order: i });
    }
    for (const list of Object.values(files)) list.sort((x, y) => x.order - y.order);
    const order = ["mac", "windows", "linux"].sort((a, b) => (b === here) - (a === here));
    if (!Object.keys(files).length) {
      const what = product === "desktop" ? "Canopy Desktop arrived in v1.0.2 (Windows in v1.0.3)" : "This release has no terminal app files";
      body.innerHTML = `<p class="dl-empty">${what}. Pick a newer version above.</p>`;
    } else {
      body.innerHTML = `<div class="dl-grid">${order
        .map((os) => {
          const list = files[os] || [];
          return `<section class="dl-os${os === here ? " mine" : ""}">
            <div class="dl-os-h"><h2>${OS_NAME[os]}</h2>${os === here ? `<span class="dl-mine">Your system</span>` : ""}</div>
            ${list.length ? commands(product, os, rel, latest, list) : ""}
            ${fileLinks(list, rel, os)}
          </section>`;
        })
        .join("")}</div>`;
    }
    const other = rel.assets.filter((a) => !known.has(a.name) && !a.name.endsWith(".sha256"));
    if (other.length) {
      body.innerHTML += `<details class="dl-other"><summary>Other files in ${esc(rel.tag)}</summary><ul>${other
        .map((a) => `<li><a href="${esc(a.url)}">${esc(a.name)}</a> <span class="faint">${mb(a.size)}</span></li>`)
        .join("")}</ul></details>`;
    }
    if (product === "desktop" && here === "mac") {
      body.innerHTML += `<p class="dl-note dl-mac-note">The Mac app is a release preview and isn't notarized by Apple yet: the first time, open <b>System Settings → Privacy &amp; Security</b> and click <b>Open Anyway</b>.</p>`;
    }
  }

  function remember() {
    const q = new URLSearchParams();
    if (product === "terminal") q.set("app", "terminal");
    if (pick.value && releases[0] && pick.value !== releases[0].tag) q.set("version", pick.value);
    const s = q.toString();
    history.replaceState(null, "", s ? "?" + s : location.pathname);
  }

  document.querySelectorAll("[data-product]").forEach((b) => {
    b.setAttribute("aria-selected", String(b.dataset.product === product));
    b.addEventListener("click", () => {
      product = b.dataset.product;
      document.querySelectorAll("[data-product]").forEach((x) => x.setAttribute("aria-selected", String(x === b)));
      remember();
      render();
    });
  });
  pick.addEventListener("change", () => {
    remember();
    render();
  });

  canopyReleases()
    .then((list) => {
      releases = list;
      // "Latest" is the newest full release, as on GitHub (not a pre-release).
      const newest = list.find((r) => !r.pre);
      pick.innerHTML = list
        .map((r) => `<option value="${esc(r.tag)}">${esc(r.tag)}${r === newest ? " (latest)" : ""}${r.pre ? " (pre-release)" : ""}</option>`)
        .join("");
      const want = params.get("version");
      if (want && list.some((r) => r.tag === want)) pick.value = want;
      pick.disabled = false;
      render();
    })
    .catch((e) => {
      body.innerHTML = `<p class="dl-empty">Couldn't load the releases from GitHub (${esc(e.message)}). Every file is on <a href="${REPO}/releases">GitHub releases</a>.</p>`;
      pick.innerHTML = "<option>—</option>";
    });
})();
