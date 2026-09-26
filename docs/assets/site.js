// Canopy site: screenshot gallery, frame scaling, copy buttons, active nav.

// Gallery (home page): one tab per frame inside #screen.
const screen = document.getElementById("screen");
const tabs = document.getElementById("shot-tabs");
const title = document.getElementById("shot-title");
if (screen && tabs) {
  const shots = [...screen.querySelectorAll("pre.frame")];
  const show = id => {
    shots.forEach(f => (f.hidden = f.id !== id));
    [...tabs.children].forEach(b => b.setAttribute("aria-selected", String(b.dataset.id === id)));
    const f = document.getElementById(id);
    if (f && title) title.textContent = "canopy — " + f.dataset.label;
    fitAll();
  };
  shots.forEach(f => {
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "tab");
    b.dataset.id = f.id;
    b.textContent = f.dataset.label;
    b.addEventListener("click", () => show(f.id));
    tabs.appendChild(b);
  });
  if (shots.length) show(shots[0].id);
}

// Scale each visible terminal frame so its widest line fits its container.
function fitAll() {
  document.querySelectorAll(".screen").forEach(box => {
    const frame = [...box.querySelectorAll("pre.frame")].find(f => !f.hidden);
    if (!frame) return;
    const cols = Math.max(...frame.textContent.split("\n").map(l => l.length));
    const avail = box.clientWidth - 28;
    const size = Math.max(6.5, Math.min(13, avail / (cols * 0.602)));
    box.querySelectorAll("pre.frame").forEach(f => (f.style.fontSize = size + "px"));
  });
}
addEventListener("resize", fitAll);
if (document.fonts) document.fonts.ready.then(fitAll);
fitAll();

// Copy buttons: data-copy-text, or data-copy="<element id>".
document.querySelectorAll(".copy").forEach(btn => {
  btn.addEventListener("click", async () => {
    const text = btn.dataset.copyText || document.getElementById(btn.dataset.copy)?.textContent || "";
    try {
      await navigator.clipboard.writeText(text.trim());
      btn.textContent = "Copied";
      btn.classList.add("done");
      setTimeout(() => {
        btn.textContent = "Copy";
        btn.classList.remove("done");
      }, 1600);
    } catch {
      /* Clipboard blocked: the text stays selectable. */
    }
  });
});

// Highlight the current page in the nav.
const here = location.pathname.split("/").pop() || "index.html";
document.querySelectorAll("nav.top .links a").forEach(a => {
  if (a.getAttribute("href") === here) a.setAttribute("aria-current", "page");
});
