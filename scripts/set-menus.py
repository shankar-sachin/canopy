#!/usr/bin/env python3
"""Rewrite the site header's menus in every docs/*.html page (root pages and, with --wiki-home, docs/wiki/index.html).
The wiki's other pages copy their header from the wiki home via scripts/wiki.mjs."""
import re, sys, glob, os

CARET = '<svg class="caret" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>'
MENUS = [
    ("Product", [
        ("desktop.html", "canopy desktop", "Start here: Mac, Windows and Linux"),
        ("features.html", "canopy console", "Everything, from the keyboard in a terminal"),
        ("download.html", "Downloads", "Every installer, every version"),
        ("github.html", "GitHub", "Pull requests, issues and CI"),
        ("changelog.html", "Changelog", "What's new in each release"),
    ]),
    ("Docs", [
        ("get-started.html", "Get started", "Install and first run"),
        ("wiki/index.html", "Wiki", "Two guides, and the git basics"),
        ("wiki/desktop/index.html", "canopy desktop guide", "Every page and recipe, with screenshots"),
        ("wiki/console/index.html", "canopy console guide", "Every screen and recipe, by key"),
        ("keys.html", "Console keys", "Every key, generated from the app"),
        ("wiki/console/config.html", "Console configuration", "Every setting in config.toml"),
    ]),
    ("Learn", [
        ("wiki/git-basics.html", "Git in ten minutes", "The ideas that make git click"),
        ("wiki/desktop/daily-workflow.html", "Daily workflow: desktop", "Edit, stage, commit, push, PR"),
        ("wiki/console/daily-workflow.html", "Daily workflow: console", "The same loop, from the keyboard"),
        ("wiki/desktop/undo.html", "Undo a mistake: desktop", "Get back anything with Undo"),
        ("wiki/console/undo.html", "Undo a mistake: console", "Get back anything with z and the reflog"),
    ]),
]

def nav(prefix, current):
    out = []
    for i, (name, items) in enumerate(MENUS):
        active = any(prefix + h == current for h, _, _ in items)
        links = "".join(
            f'<a href="{prefix}{h}"{" aria-current=\"page\"" if prefix + h == current else ""}><b>{t}</b><span>{d}</span></a>'
            for h, t, d in items)
        out.append(f'<div class="menu{" active" if active else ""}"><button type="button" aria-expanded="false" aria-controls="menu-{i}">{name}{CARET}</button><div class="dropdown" id="menu-{i}">{links}</div></div>')
    return '<nav class="menus" id="menus" aria-label="Main">' + "".join(out) + "</nav>"

files = sorted(glob.glob("docs/*.html"))
if "--wiki-home" in sys.argv:
    files.append("docs/wiki/index.html")
for f in files:
    s = open(f).read()
    prefix = "../" if f.startswith("docs/wiki/") else ""
    current = prefix + os.path.basename(f) if not f.startswith("docs/wiki/") else "../wiki/index.html"
    new = re.sub(r'<nav class="menus" id="menus" aria-label="Main">.*?</nav>', lambda m: nav(prefix, current), s, count=1, flags=re.S)
    if new != s:
        open(f, "w").write(new)
        print("menus:", f)
