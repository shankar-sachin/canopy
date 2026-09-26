// Branches: local and remote branches, tags, and remotes.
"use strict";

const br = {
  tab: "local",
  tags: [],
};

function trackPills(b) {
  if (b.track === "gone") return `<span class="pill red" title="The remote branch was deleted">gone</span>`;
  let out = "";
  const a = /ahead (\d+)/.exec(b.track || "");
  const bh = /behind (\d+)/.exec(b.track || "");
  if (a) out += `<span class="pill teal" title="Commits to push">↑ <b>${a[1]}</b></span>`;
  if (bh) out += `<span class="pill amber" title="Commits to pull">↓ <b>${bh[1]}</b></span>`;
  return out;
}

function branchesHtml(o) {
  const head = o.status.branch.head;
  const locals = o.branches.filter((b) => !b.is_remote).sort((a, b) => b.is_head - a.is_head || b.time - a.time);
  const remotes = o.branches.filter((b) => b.is_remote && !b.name.endsWith("/HEAD")).sort((a, b) => b.time - a.time);
  const tabs = [["local", "Local", locals.length], ["remote", "Remote", remotes.length], ["tags", "Tags", br.tags.length], ["remotes", "Remotes", o.remotes.length]];
  const tabBar = `<div class="tabs">${tabs.map(([id, l, n]) => `<button class="tab${br.tab === id ? " on" : ""}" data-b="tab" data-tab="${id}">${l} <span class="badge">${n}</span></button>`).join("")}
    <span class="grow"></span><button class="btn small primary" data-b="new">New branch…</button></div>`;
  let rows = "";
  const act = (op, label, extra = "") => `<button class="btn small ghost" data-b="${op}"${extra}>${label}</button>`;
  if (br.tab === "local") {
    rows = locals.map((b) => `<li class="brow${b.is_head ? " head" : ""}" data-name="${esc(b.name)}">
      <span class="bname">${b.is_head ? `<span class="cur" title="You are here">●</span>` : ""}${esc(b.name)}</span>${trackPills(b)}
      <span class="bsubj">${esc(b.subject)}</span><span class="htime">${ago(b.time)}</span>
      <span class="bacts">${b.is_head ? act("rename", "Rename") : act("checkout", "Check out") + act("merge", `Merge into ${esc(head || "HEAD")}`) + act("rebase", "Rebase onto") + act("rename", "Rename") + act("delete", "Delete")}</span>
    </li>`).join("");
  } else if (br.tab === "remote") {
    rows = remotes.map((b) => `<li class="brow" data-name="${esc(b.name)}">
      <span class="bname remote">${esc(b.name)}</span><span class="bsubj">${esc(b.subject)}</span><span class="htime">${ago(b.time)}</span>
      <span class="bacts">${act("checkout-remote", "Check out")}${act("merge", `Merge into ${esc(head || "HEAD")}`)}${act("delete-remote", "Delete on remote")}</span>
    </li>`).join("");
  } else if (br.tab === "tags") {
    rows = br.tags.map((t) => `<li class="brow" data-name="${esc(t.name)}">
      <span class="bname tag">${esc(t.name)}</span><span class="bsubj">${esc(t.subject)}</span><span class="htime">${ago(t.time)}</span>
      <span class="bacts">${act("checkout", "Check out")}${o.remotes.length ? act("push-tag", "Push") : ""}${act("delete-tag", "Delete")}</span>
    </li>`).join("");
  } else {
    rows = o.remotes.map((r) => `<li class="brow" data-name="${esc(r.name)}">
      <span class="bname remote">${esc(r.name)}</span><span class="bsubj mono selectable">${esc(r.fetch_url)}</span>
    </li>`).join("");
  }
  const empty = { local: "No branches yet: make a first commit.", remote: "No remote branches. Fetch, or push a branch.", tags: "No tags yet. Tag a commit from History.", remotes: "No remotes. Add one with: git remote add origin <url>" }[br.tab];
  return `${tabBar}<ul class="blist">${rows || `<li class="diff-empty">${empty}</li>`}</ul>`;
}

async function loadTags() {
  const refs = await invoke("refs").catch(() => ({ tags: [], stashes: [] }));
  br.tags = refs.tags;
  if (state.page === "branches") $("#branches").innerHTML = branchesHtml(state.overview);
}

async function branchAction(what, el) {
  const o = state.overview;
  const name = el.closest(".brow")?.dataset.name;
  const head = o.status.branch.head || "HEAD";
  switch (what) {
    case "tab":
      br.tab = el.dataset.tab;
      $("#branches").innerHTML = branchesHtml(o);
      return;
    case "new": {
      const f = await prompt({ title: "New branch", text: `Starts from ${head}.`, fields: [
        { id: "name", label: "Branch name", placeholder: "feature/my-idea" },
        { id: "switch", label: "Switch to it", type: "checkbox", value: true },
      ], ok: "Create branch" });
      if (f?.name) return run(`Created ${f.name}`, "git_op", { op: "create-branch", args: [f.name, "", f.switch ? "switch" : ""] });
      return;
    }
    case "checkout":
      return run(`Checked out ${name}`, "git_op", { op: "checkout", args: [name] });
    case "checkout-remote":
      return run(`Checked out ${name}`, "git_op", { op: "checkout-remote", args: [name] });
    case "merge": {
      const ok = await ask({ title: `Merge ${name} into ${head}?`, text: `Brings ${name}'s commits into ${head}. If both changed the same lines you'll resolve conflicts in Changes.`,
        buttons: [{ label: "Merge", value: true, kind: "primary" }] });
      if (ok) return run(`Merged ${name}`, "git_op", { op: "merge", args: [name] });
      return;
    }
    case "rebase": {
      const ok = await ask({ title: `Rebase ${head} onto ${name}?`, text: `Replays ${head}'s own commits on top of ${name}, as if you'd started from there. Don't rebase commits others already pulled.`,
        buttons: [{ label: "Rebase", value: true, kind: "primary" }] });
      if (ok) return run(`Rebased onto ${name}`, "git_op", { op: "rebase", args: [name] });
      return;
    }
    case "rename": {
      const f = await prompt({ title: `Rename ${name}`, fields: [{ id: "to", label: "New name", value: name }], ok: "Rename" });
      if (f?.to && f.to !== name) return run(`Renamed ${name} to ${f.to}`, "git_op", { op: "rename-branch", args: [name, f.to] });
      return;
    }
    case "delete": {
      const b = o.branches.find((x) => x.name === name && !x.is_remote);
      const unpushed = !b?.upstream || /ahead/.test(b?.track || "");
      const how = await ask({
        title: `Delete ${name}?`,
        text: unpushed
          ? "Git refuses if its commits aren't merged anywhere. Force delete skips that check (the reflog can still find them for a while)."
          : "Its commits are on the remote, so nothing is lost.",
        buttons: [{ label: "Delete", value: "safe", kind: "primary" }, { label: "Force delete", value: "force", kind: "danger" }],
      });
      if (how) return run(`Deleted ${name}`, "git_op", { op: "delete-branch", args: [name, how === "force" ? "force" : ""] });
      return;
    }
    case "delete-remote": {
      const [remote, ...rest] = name.split("/");
      const ok = await ask({ title: `Delete ${name} on ${remote}?`, text: "This deletes the branch on the remote for everyone. Your local branches stay.",
        buttons: [{ label: "Delete on remote", value: true, kind: "danger" }] });
      if (ok) return run(`Deleted ${name}`, "git_op", { op: "delete-remote-branch", args: [remote, rest.join("/")] });
      return;
    }
    case "push-tag": {
      const remote = o.remotes.find((r) => r.name === "origin")?.name || o.remotes[0].name;
      return run(`Pushed tag ${name} to ${remote}`, "git_op", { op: "push-tag", args: [remote, name] }).then(loadTags);
    }
    case "delete-tag": {
      const ok = await ask({ title: `Delete tag ${name}?`, text: "Deletes it here only; a pushed tag stays on the remote.", buttons: [{ label: "Delete", value: true, kind: "danger" }] });
      if (ok) return run(`Deleted tag ${name}`, "git_op", { op: "delete-tag", args: [name] }).then(loadTags);
    }
  }
}

addPage("branches", {
  title: "Branches",
  icon: "branches",
  badge: (o) => {
    const n = o.branches.filter((b) => !b.is_remote).length;
    return n > 1 ? `<span class="badge">${n}</span>` : "";
  },
  render: (o) => `<div class="card branches-card" id="branches">${branchesHtml(o)}</div>`,
  mounted: (o, view) => {
    view.addEventListener("click", (e) => {
      const act = e.target.closest("[data-b]");
      if (act) branchAction(act.dataset.b, act);
    });
    loadTags();
  },
  update: (o) => {
    $("#branches").innerHTML = branchesHtml(o);
  },
});

ACTIONS.branches = { label: "Open Branches", run: () => go("branches") };
