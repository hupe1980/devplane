/* Vibeplane docs — the only script on the site.
 *
 * Four jobs: the theme toggle, the documentation search, copy buttons on code
 * blocks, and the mobile menu. Everything else is HTML the server already sent,
 * because a documentation page that cannot render without JavaScript is a
 * documentation page that is sometimes blank.
 *
 * The search index is fetched lazily — on the first keystroke — so a reader who
 * never searches never pays for it. */

(function () {
  "use strict";

  // ── theme ──────────────────────────────────────────────────────────────────
  // Three states, not two: dark, light, and "whatever the system says", which
  // is the default and the one a toggle usually throws away. Clicking cycles
  // back to it rather than trapping the reader in a choice they made once.
  var root = document.documentElement;
  var toggle = document.getElementById("theme-toggle");

  if (toggle) {
    toggle.addEventListener("click", function () {
      var systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      var current = root.dataset.theme || (systemDark ? "dark" : "light");
      var next = current === "dark" ? "light" : "dark";

      // Back to the system preference when the choice would agree with it.
      var chosen = (next === "dark") === systemDark ? null : next;
      window.vpApplyTheme(chosen);
      try {
        if (chosen) localStorage.setItem("vp-theme", chosen);
        else localStorage.removeItem("vp-theme");
      } catch (e) {}
    });
  }

  // ── mobile menu ────────────────────────────────────────────────────────────
  var navToggle = document.getElementById("nav-toggle");
  if (navToggle) {
    navToggle.addEventListener("click", function () {
      var open = document.body.classList.toggle("nav-open");
      navToggle.setAttribute("aria-expanded", String(open));
    });
  }

  // ── copy buttons ───────────────────────────────────────────────────────────
  // On the code blocks Zola generated, and on the hand-written install lines.
  document.querySelectorAll(".doc pre").forEach(function (pre) {
    var wrap = document.createElement("div");
    wrap.className = "code-wrap";
    pre.parentNode.insertBefore(wrap, pre);
    wrap.appendChild(pre);

    var btn = document.createElement("button");
    btn.type = "button";
    btn.className = "copy-btn";
    btn.textContent = "Copy";
    btn.setAttribute("aria-label", "Copy this code");
    wrap.appendChild(btn);
    btn.addEventListener("click", function () { copy(pre.innerText, btn); });
  });

  document.querySelectorAll("[data-copy]").forEach(function (btn) {
    btn.addEventListener("click", function () { copy(btn.dataset.copy, btn); });
  });

  function copy(text, btn) {
    // A shell prompt is not part of the command. Copying `$ cargo install x`
    // and pasting it produces "command not found: $".
    var clean = text.replace(/^\s*\$ /gm, "").trim();
    var done = function () {
      var was = btn.textContent;
      btn.textContent = "Copied";
      setTimeout(function () { btn.textContent = was; }, 1400);
    };
    if (navigator.clipboard) {
      navigator.clipboard.writeText(clean).then(done, function () {});
      return;
    }
    var ta = document.createElement("textarea");
    ta.value = clean;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); done(); } catch (e) {}
    document.body.removeChild(ta);
  }

  // ── search ─────────────────────────────────────────────────────────────────
  var input = document.getElementById("search-input");
  var results = document.getElementById("search-results");
  var index = null;
  var loading = false;

  if (input && results) {
    input.addEventListener("input", function () {
      var q = input.value.trim();
      if (q.length < 2) { hide(); return; }
      ensureIndex().then(function () { render(q); });
    });

    input.addEventListener("keydown", function (e) {
      if (e.key === "Escape") { input.value = ""; hide(); input.blur(); }
      if (e.key === "ArrowDown" || e.key === "Enter") {
        var first = results.querySelector("a");
        if (first) { e.preventDefault(); first.focus(); }
      }
    });

    results.addEventListener("keydown", function (e) {
      var links = Array.prototype.slice.call(results.querySelectorAll("a"));
      var at = links.indexOf(document.activeElement);
      if (e.key === "ArrowDown") { e.preventDefault(); (links[at + 1] || links[0]).focus(); }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        if (at <= 0) input.focus(); else links[at - 1].focus();
      }
      if (e.key === "Escape") { hide(); input.focus(); }
    });

    document.addEventListener("click", function (e) {
      if (!results.contains(e.target) && e.target !== input) hide();
    });

    // `/` focuses search, the way it does in most documentation people already
    // read — but never while they are typing somewhere else.
    document.addEventListener("keydown", function (e) {
      if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
      var t = e.target;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
      e.preventDefault();
      document.body.classList.add("nav-open");
      input.focus();
      input.select();
    });
  }

  function ensureIndex() {
    if (index) return Promise.resolve(index);
    if (loading) return loading;
    loading = fetch(withBase("search_index.en.json"))
      .then(function (r) { return r.json(); })
      .then(function (data) {
        // Zola's elasticlunr index, read directly: `documentStore.docs` maps a
        // permalink to { id, path, title, description, body }. A substring pass
        // over a few dozen pages is all this needs, and loading the elasticlunr
        // runtime to rank them would be a library fetched per reader who types.
        var store = data && data.documentStore;
        var docs = (store && store.docs) || {};
        index = Object.keys(docs)
          .map(function (k) { return docs[k]; })
          // The landing page has no body worth searching and its title is the
          // site name, so it matches everything and helps nobody.
          .filter(function (d) { return d && d.path !== "/"; });
        return index;
      })
      .catch(function () { index = []; return index; });
    return loading;
  }

  function render(q) {
    var needle = q.toLowerCase();
    var hits = [];

    for (var i = 0; i < index.length && hits.length < 40; i++) {
      var d = index[i];
      var title = (d.title || "").toLowerCase();
      var body = (d.body || "").toLowerCase();
      var at = title.indexOf(needle);
      if (at > -1) { hits.push({ doc: d, rank: at === 0 ? 0 : 1, where: null }); continue; }
      var bodyAt = body.indexOf(needle);
      if (bodyAt > -1) hits.push({ doc: d, rank: 2, where: bodyAt });
    }

    hits.sort(function (a, b) { return a.rank - b.rank; });
    hits = hits.slice(0, 8);

    if (!hits.length) {
      results.innerHTML = "<p>No matches for “" + escapeHtml(q) + "”.</p>";
      results.hidden = false;
      return;
    }

    results.innerHTML = hits.map(function (h) {
      var d = h.doc;
      var context = h.where === null
        ? (d.description || "")
        : excerpt(d.body || "", h.where);
      return '<a href="' + escapeHtml(d.id) + '">' +
             escapeHtml(d.title || d.id) +
             (context ? "<span>" + escapeHtml(context) + "</span>" : "") +
             "</a>";
    }).join("");
    results.hidden = false;
  }

  function excerpt(body, at) {
    var from = Math.max(0, at - 45);
    var text = body.slice(from, from + 150).replace(/\s+/g, " ").trim();
    return (from > 0 ? "…" : "") + text + "…";
  }

  function hide() { results.hidden = true; results.innerHTML = ""; }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
    });
  }

  function withBase(file) {
    // The site may be served from a sub-path; the script element knows where
    // it came from, so the index is found relative to that rather than guessed.
    var src = document.currentScript && document.currentScript.src;
    if (!src) {
      var tags = document.getElementsByTagName("script");
      src = tags[tags.length - 1].src;
    }
    return src.replace(/site\.js(\?.*)?$/, file);
  }

  // ── table of contents ──────────────────────────────────────────────────────
  // Marks the heading currently on screen. `rootMargin` puts the trigger line
  // near the top, so the highlight tracks what is being read rather than what
  // is about to scroll away.
  var tocLinks = document.querySelectorAll(".toc a");
  if (tocLinks.length && "IntersectionObserver" in window) {
    var byId = {};
    tocLinks.forEach(function (a) {
      var id = decodeURIComponent((a.getAttribute("href") || "").split("#")[1] || "");
      if (id) byId[id] = a;
    });

    var seen = [];
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        var id = entry.target.id;
        var i = seen.indexOf(id);
        if (entry.isIntersecting && i === -1) seen.push(id);
        if (!entry.isIntersecting && i > -1) seen.splice(i, 1);
      });
      tocLinks.forEach(function (a) { a.classList.remove("is-active"); });
      if (seen.length && byId[seen[0]]) byId[seen[0]].classList.add("is-active");
    }, { rootMargin: "-72px 0px -70% 0px" });

    Object.keys(byId).forEach(function (id) {
      var el = document.getElementById(id);
      if (el) observer.observe(el);
    });
  }
})();
