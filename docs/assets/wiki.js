// Canopy wiki: "On this page" and the phone "Wiki pages" button.
(() => {
  // ------------------------------------------------------- on this page
  const toc = document.getElementById("toc");
  // Only real section headings in the article body (not headings in cards).
  const heads = [...document.querySelectorAll(".wprose > h2[id], .wprose > h3[id]")];
  if (toc) {
    if (!heads.length) toc.closest(".wtoc").style.visibility = "hidden";
    heads.forEach(h => {
      const a = document.createElement("a");
      a.href = "#" + h.id;
      a.textContent = h.firstChild?.textContent?.trim() || h.textContent.replace(/#$/, "").trim();
      if (h.tagName === "H3") a.className = "sub";
      toc.appendChild(a);
    });
    const links = [...toc.querySelectorAll("a")];
    const setActive = () => {
      let current = heads[0];
      for (const h of heads) if (h.getBoundingClientRect().top <= 120) current = h;
      links.forEach(l => l.classList.toggle("active", !!current && l.getAttribute("href") === "#" + current.id));
    };
    addEventListener("scroll", setActive, { passive: true });
    setActive();
  }

  // ------------------------------------------------ phones: page list
  const main = document.querySelector(".wmain");
  if (main) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "wpages-btn";
    btn.setAttribute("aria-controls", "wside");
    btn.setAttribute("aria-expanded", "false");
    btn.textContent = "☰  Wiki pages";
    main.insertBefore(btn, main.querySelector(".wprose"));
    btn.addEventListener("click", e => {
      e.stopPropagation();
      const open = document.body.classList.toggle("nav-open");
      btn.setAttribute("aria-expanded", String(open));
    });
    document.querySelector(".wprose")?.addEventListener("click", () => {
      document.body.classList.remove("nav-open");
      btn.setAttribute("aria-expanded", "false");
    });
  }
})();
