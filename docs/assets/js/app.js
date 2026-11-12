/* ============================================================================
 * app.js — UI controller: fetch GitHub data, compute the snapshot, drive the
 * renderer, stats panel, stage carousel, downloads and embed snippets.
 * ========================================================================== */
(function () {
  'use strict';

  var $ = function (sel) { return document.querySelector(sel); };
  var $$ = function (sel) { return Array.prototype.slice.call(document.querySelectorAll(sel)); };

  var SITE_URL = 'https://codewithsyedabdullah.github.io/git-dragon/';
  var FRAME_MS = 70; // 7 hundredths of a second, like the GIF

  var state = {
    username: null,
    snapshot: null,
    stage: null,
    coat: null,
    rig: null,
    lastT: -1,
    animating: false,
    live: false,
  };

  /* ------------------------------------------------------------------ *
   * Tiny utilities                                                      *
   * ------------------------------------------------------------------ */

  function fmt(n) { return Number(n).toLocaleString('en-US'); }

  function clamp01(v) { return v < 0 ? 0 : v > 1 ? 1 : v; }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }

  function toast(msg) {
    var el = $('#toast');
    el.textContent = msg;
    el.classList.add('show');
    clearTimeout(toast._t);
    toast._t = setTimeout(function () { el.classList.remove('show'); }, 2600);
  }

  function copyText(text, label) {
    function done() {
      toast((label || 'Copied') + ' ✓');
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () { fallbackCopy(text); done(); });
    } else {
      fallbackCopy(text);
      done();
    }
  }

  function fallbackCopy(text) {
    var ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand('copy'); } catch (e) { /* ignore */ }
    document.body.removeChild(ta);
  }

  function downloadCanvas(canvas, filename) {
    canvas.toBlob(function (blob) {
      if (!blob) return;
      var a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      setTimeout(function () { URL.revokeObjectURL(a.href); }, 5000);
    }, 'image/png');
  }

  function setStatus(msg) {
    var el = $('#gen-status');
    if (el) el.textContent = msg || '';
  }

  /* ------------------------------------------------------------------ *
   * GitHub data fetching (all client-side, CORS-enabled endpoints)      *
   * ------------------------------------------------------------------ */

  function fetchJson(url, timeoutMs) {
    var ctrl = new AbortController();
    var timer = setTimeout(function () { ctrl.abort(); }, timeoutMs || 15000);
    return fetch(url, {
      signal: ctrl.signal,
      headers: { Accept: 'application/vnd.github+json' },
    }).then(function (r) {
      clearTimeout(timer);
      if (!r.ok) {
        var err = new Error('HTTP ' + r.status);
        err.status = r.status;
        throw err;
      }
      return r.json();
    });
  }

  /* Top 100 owner repos by updated — the same window the Rust action uses,
   * and a single tokenless API call per generate (rate-limit friendly). */
  function fetchRepos(login) {
    return fetchJson('https://api.github.com/users/' + encodeURIComponent(login) +
      '/repos?per_page=100&type=owner&sort=updated').then(function (batch) {
      return Array.isArray(batch) ? batch : [];
    });
  }

  /* ------------------------------------------------------------------ *
   * Snapshot cache (localStorage, 1h TTL) — repeat generates cost zero *
   * API calls, which matters on shared/rate-limited IPs.                *
   * ------------------------------------------------------------------ */
  var CACHE_TTL_MS = 60 * 60 * 1000;
  function cacheKey(login) { return 'git-dragon:' + login; }
  function readCache(login) {
    try {
      var raw = localStorage.getItem(cacheKey(login));
      if (!raw) return null;
      var o = JSON.parse(raw);
      if (!o || !o.ts || !o.snapshot) return null;
      var age = Date.now() - o.ts;
      if (age > CACHE_TTL_MS) { localStorage.removeItem(cacheKey(login)); return null; }
      return { snapshot: o.snapshot, ageMin: Math.max(1, Math.round(age / 60000)) };
    } catch (e) { return null; }
  }
  function writeCache(login, snapshot) {
    try { localStorage.setItem(cacheKey(login), JSON.stringify({ ts: Date.now(), snapshot: snapshot })); } catch (e) { /* private mode */ }
  }

  /* ------------------------------------------------------------------ *
   * Snapshot computation (history.rs + pipeline.rs, Annual window)      *
   * ------------------------------------------------------------------ */

  function computeSnapshot(login, userJson, repos, contribsJson) {
    var now = new Date();
    var y = now.getUTCFullYear();
    var m = now.getUTCMonth() + 1;
    var monthPrefix = y + '-' + (m < 10 ? '0' : '') + m;
    var yearStartMs = Date.UTC(y, 0, 1);
    var todayMs = Date.UTC(y, now.getUTCMonth(), now.getUTCDate());

    var total = 0;
    var monthly = 0;
    var current = 0;
    var longest = 0;
    var running = 0;
    var days = (contribsJson && contribsJson.contributions) || [];
    for (var i = 0; i < days.length; i++) {
      var d = days[i];
      if (!d || !d.date) continue;
      var t = Date.parse(d.date + 'T00:00:00Z');
      if (isNaN(t) || t < yearStartMs || t > todayMs) continue;
      var count = d.count || 0;
      total += count;
      if (d.date.indexOf(monthPrefix) === 0) monthly += count;
      if (count > 0) { running++; current = running; } else { running = 0; }
      if (current > longest) longest = current;
    }

    /* rank_languages: primary language of non-fork repos, top 12 */
    var langMap = {};
    var repoCount = 0;
    for (var r = 0; r < repos.length; r++) {
      var repo = repos[r];
      if (repo.fork || !repo.language) continue;
      repoCount++;
      if (!langMap[repo.language]) {
        langMap[repo.language] = {
          name: repo.language,
          hex_color: window.DragonData.languageHex(repo.language),
          repo_count: 0,
        };
      }
      langMap[repo.language].repo_count++;
    }
    var languages = Object.keys(langMap).map(function (k) { return langMap[k]; });
    languages.sort(function (a, b) { return b.repo_count - a.repo_count; });
    languages = languages.slice(0, 12);

    /* Bonus stat: all-time total from the contributions API per-year table. */
    var allTime = 0;
    if (contribsJson && contribsJson.total) {
      Object.keys(contribsJson.total).forEach(function (yr) { allTime += contribsJson.total[yr] || 0; });
    }

    return {
      login: login,
      account_id: userJson ? userJson.id || 1 : 1,
      observed_on: now.toISOString().slice(0, 10),
      total_contributions: total,
      monthly_contributions: monthly,
      current_streak: current,
      longest_streak: longest,
      languages: languages,
      all_time: allTime,
      repo_count: repoCount,
      profile: userJson || null,
    };
  }

  /* ------------------------------------------------------------------ *
   * Rendering                                                           *
   * ------------------------------------------------------------------ */

  var mainCanvas = null;
  var mainCtx = null;

  function paintMain(t) {
    var rig = state.rig;
    if (!rig || !mainCtx) return;
    var frame = DragonRenderer.renderFrame(rig, t);
    var img = mainCtx.createImageData(DragonRenderer.CANVAS_W, DragonRenderer.CANVAS_H);
    img.data.set(frame.data);
    mainCtx.putImageData(img, 0, 0);
  }

  function startAnimation() {
    if (state.animating) return;
    state.animating = true;
    state.lastT = -1;
    function loop(now) {
      if (!state.rig) { state.animating = false; return; }
      var t = Math.floor(now / FRAME_MS) % DragonRenderer.LOOP_FRAMES;
      if (t !== state.lastT) {
        state.lastT = t;
        paintMain(t);
        var fc = $('#frame-counter');
        if (fc) fc.textContent = 'frame ' + (t + 1) + ' / ' + DragonRenderer.LOOP_FRAMES;
      }
      requestAnimationFrame(loop);
    }
    requestAnimationFrame(loop);
  }

  function hexOf(rgb) {
    return '#' + rgb.map(function (c) { return c.toString(16).padStart(2, '0'); }).join('');
  }

  function renderSnapshot(snapshot, opts) {
    opts = opts || {};
    state.snapshot = snapshot;
    state.live = !!opts.live;

    var n = Math.max(snapshot.total_contributions, snapshot.monthly_contributions);
    var stageSlug = DragonRenderer.stageFromContributions(n);
    var coat = DragonRenderer.coatFromLanguages(snapshot.languages || []);
    state.stage = stageSlug;
    state.coat = coat;
    state.rig = DragonRenderer.rig(stageSlug, coat);

    var stageMeta = null;
    DragonData.STAGES.forEach(function (s) { if (s.slug === stageSlug) stageMeta = s; });

    /* Header */
    $('#dragon-login').textContent = '@' + snapshot.login;
    /* Avatar: exact API url when we have it; for live users without profile
     * data (rate-limited) fall back to github.com's .png endpoint — no API
     * quota. Demo snapshots keep the 🥚 fallback. */
    var avatar = $('#dragon-avatar');
    var fallback = $('#dragon-avatar-fallback');
    avatar.onload = function () { avatar._ok = true; avatar.style.display = ''; fallback.style.display = 'none'; };
    avatar.onerror = function () { avatar._ok = false; avatar.style.display = 'none'; fallback.style.display = ''; };
    var avSrc = null;
    if (snapshot.profile && snapshot.profile.avatar_url) avSrc = snapshot.profile.avatar_url;
    else if (state.live) avSrc = 'https://github.com/' + encodeURIComponent(snapshot.login) + '.png';
    if (avSrc) {
      var asep = avSrc.indexOf('?') >= 0 ? '&' : '?';
      var afull = avSrc + asep + 's=96';
      if (avatar._ok && avatar.getAttribute('src') === afull) {
        avatar.style.display = '';
        fallback.style.display = 'none';
      } else {
        avatar._ok = false;
        avatar.style.display = 'none';
        fallback.style.display = '';
        avatar.src = afull;
      }
    } else {
      avatar.style.display = 'none';
      fallback.style.display = '';
    }

    var who = $('#dragon-who');
    if (snapshot.profile) {
      var name = snapshot.profile.name || snapshot.profile.login;
      who.textContent = name + (snapshot.profile.bio ? ' — ' + snapshot.profile.bio : '');
      who.title = name;
    } else if (state.live) {
      who.textContent = 'profile API unavailable (rate limit) — showing public activity only';
    } else {
      who.textContent = 'bundled demo snapshot — type a username for the real thing';
    }

    /* Chips */
    var stageChip = $('#chip-stage');
    stageChip.textContent = stageMeta.title;
    stageChip.className = 'chip chip-stage chip-stage-' + stageSlug;
    $('#chip-fire').style.display = DragonRenderer.breathesFire(stageSlug) ? '' : 'none';
    var coatChip = $('#chip-coat');
    coatChip.style.display = coat ? '' : 'none';
    if (coat) {
      coatChip.innerHTML = '';
      [coat.primary, coat.secondary, coat.tertiary].forEach(function (tint) {
        var dot = document.createElement('span');
        dot.className = 'chip-dot';
        dot.style.background = hexOf(tint.base);
        dot.title = tint.language;
        coatChip.appendChild(dot);
      });
    }

    /* Stats */
    $('#stat-total').textContent = fmt(snapshot.total_contributions);
    $('#stat-monthly').textContent = fmt(snapshot.monthly_contributions);
    $('#stat-current').textContent = fmt(snapshot.current_streak) + (snapshot.current_streak === 1 ? ' day' : ' days');
    $('#stat-longest').textContent = fmt(snapshot.longest_streak) + (snapshot.longest_streak === 1 ? ' day' : ' days');
    $('#extra-stats').textContent = '· ' + fmt(snapshot.all_time || 0) + ' lifetime · ' + fmt(snapshot.repo_count || 0) + ' repos';

    /* Progress to next stage */
    var lo = stageMeta.lo, hi = stageMeta.hi;
    var pct, progressLabel;
    var next = DragonData.STAGES[DragonRenderer.stageIndex(stageSlug) + 1];
    if (hi === null) {
      pct = 1;
      progressLabel = 'MAX — the legend continues';
    } else {
      /* Egg: lo === hi === 0 — show progress toward hatching instead of 0/0. */
      pct = hi === lo ? 0 : (n - lo) / (hi - lo);
      var remaining = (next ? next.lo : hi + 1) - n;
      progressLabel = remaining > 0
        ? fmt(remaining) + (remaining === 1 ? ' more contribution to ' : ' more contributions to ') + (next ? next.name : 'the next stage')
        : 'ready to evolve!';
    }
    $('#progress-fill').style.width = Math.round(clamp01(pct) * 100) + '%';
    $('#progress-label').textContent = progressLabel;
    $('#progress-count').textContent = hi === null
      ? fmt(n) + ' +'
      : (hi === lo && next ? fmt(n) + ' / ' + fmt(next.lo) : fmt(n) + ' / ' + fmt(hi));

    /* Language chips (top 5) */
    var langBox = $('#lang-chips');
    langBox.innerHTML = '';
    (snapshot.languages || []).slice(0, 5).forEach(function (l) {
      var chip = document.createElement('span');
      chip.className = 'chip chip-lang';
      var dot = document.createElement('span');
      dot.className = 'chip-dot';
      dot.style.background = l.hex_color;
      chip.appendChild(dot);
      chip.appendChild(document.createTextNode(l.name + ' × ' + l.repo_count));
      langBox.appendChild(chip);
    });
    if (!langBox.children.length) {
      langBox.innerHTML = '<span class="muted">No public repo languages — canonical emerald coat.</span>';
    }

    /* Coat zones */
    var zones = $('#coat-zones');
    zones.innerHTML = '';
    if (coat) {
      [['Primary', coat.primary, 'scales · belly · highlights'],
       ['Secondary', coat.secondary, 'wing membranes'],
       ['Tertiary', coat.tertiary, 'horns · crest']].forEach(function (z) {
        var row = document.createElement('div');
        row.className = 'coat-zone';
        var sw = document.createElement('span');
        sw.className = 'coat-swatch';
        sw.style.background = hexOf(z[1].base);
        row.appendChild(sw);
        var txt = document.createElement('div');
        txt.innerHTML = '<strong>' + z[0] + '</strong> — ' + escapeHtml(z[1].language) +
          ' <span class="muted">(' + z[2] + ', strength ' + Math.round(z[1].strength * 100) + '%)</span>';
        row.appendChild(txt);
        zones.appendChild(row);
      });
    } else {
      var canonWhy = state.live
        ? 'untouched emerald art (no usable language data)'
        : 'untouched emerald art — demo dragons are never tinted';
      zones.innerHTML = '<div class="coat-zone"><span class="coat-swatch coat-swatch-canon"></span>' +
        '<div><strong>Canonical</strong> — <span class="muted">' + canonWhy + '</span></div></div>';
    }

    /* First paint immediately — don't wait for the first rAF tick. */
    paintMain(0);
    var fc0 = $('#frame-counter');
    if (fc0) fc0.textContent = 'frame 1 / 48';

    /* Deep link + embed snippets */
    var pageUrl = state.live
      ? SITE_URL + '?user=' + encodeURIComponent(snapshot.login)
      : SITE_URL + '?demo=' + stageSlug;
    $('#embed-link').textContent = '[🐉 hatch my dragon](' + pageUrl + ')';
    $('#embed-action').textContent =
      '# .github/workflows/dragon.yml in YOUR <username>/<username> repo\n' +
      'name: Update Profile Dragon\n\n' +
      'on:\n  schedule:\n    - cron: "15 0 * * *"\n  workflow_dispatch:\n\n' +
      'permissions:\n  contents: write\n\n' +
      'jobs:\n  update-dragon:\n    runs-on: ubuntu-latest\n    steps:\n' +
      '  - uses: actions/checkout@v4\n' +
      '  - uses: codewithsyedabdullah/git-dragon@v1\n' +
      '    with:\n      window: annual\n      branch: output\n' +
      '\n' +
      '# then in your profile README:\n' +
      '# ![My dragon](https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_USERNAME/output/dragon/dragon.gif)';
    $('#profile-link').href = pageUrl;

    /* Data source footnote */
    $('#data-sources').textContent = state.live
      ? (opts.cachedAgeMin
        ? 'data: cached ' + opts.cachedAgeMin + 'm ago · github-contributions-api · api.github.com REST (tokenless)'
        : 'data: github-contributions-api · api.github.com REST (tokenless)')
      : 'data: bundled demo snapshot (works offline)';

    startAnimation();
  }

  /* ------------------------------------------------------------------ *
   * Generate flow                                                       *
   * ------------------------------------------------------------------ */

  var busy = false;

  function normalizeLogin(input) {
    var s = String(input || '').trim().replace(/^@/, '').toLowerCase();
    return /^[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?$/.test(s) ? s : null;
  }

  function showError(msg) {
    var banner = $('#dragon-error');
    banner.textContent = msg;
    banner.style.display = '';
  }

  function hideError() { $('#dragon-error').style.display = 'none'; }

  function generate(username) {
    if (busy) return;
    var login = normalizeLogin(username);
    if (!login) {
      showError('That does not look like a GitHub username (letters, numbers, dashes).');
      return;
    }
    hideError();

    /* Fresh-enough cached snapshot? Render instantly — zero API calls. */
    var cached = readCache(login);
    if (cached) {
      renderSnapshot(cached.snapshot, { live: true, cachedAgeMin: cached.ageMin });
      try { history.replaceState(null, '', '?user=' + encodeURIComponent(login)); } catch (e) { /* ignore */ }
      toast('Loaded from cache — data up to 1h old');
      return;
    }

    busy = true;
    setStatus('Contacting GitHub…');
    $('#btn-generate').disabled = true;
    $('#dragon-card').classList.add('is-loading');

    var userP = fetchJson('https://api.github.com/users/' + encodeURIComponent(login))
      .then(function (u) { return { ok: true, json: u }; })
      .catch(function (e) { return { ok: false, err: e }; });
    var contribsP = fetchJson('https://github-contributions-api.jogruber.de/v4/' + encodeURIComponent(login), 20000)
      .then(function (c) { return { ok: true, json: c }; })
      .catch(function (e) { return { ok: false, err: e }; });
    var reposP = fetchRepos(login)
      .then(function (repos) { return { ok: true, repos: repos }; })
      .catch(function () { return { ok: false, repos: [] }; });

    Promise.all([userP, contribsP, reposP]).then(function (results) {
      var userRes = results[0];
      var contribsRes = results[1];
      var reposRes = results[2];

      if (!contribsRes.ok || !contribsRes.json || !contribsRes.json.contributions) {
        if (!userRes.ok && userRes.err && userRes.err.status === 404) {
          throw new Error('not-found');
        }
        throw new Error('contributions-api');
      }

      var snapshot = computeSnapshot(login, userRes.ok ? userRes.json : null, reposRes.repos, contribsRes.json);
      writeCache(login, snapshot);
      renderSnapshot(snapshot, { live: true });
      setStatus('');

      var warnings = [];
      if (!userRes.ok) warnings.push('profile API rate-limited — name/bio hidden');
      if (!reposRes.ok) warnings.push('language data unavailable — classic emerald coat');
      if (warnings.length) toast(warnings.join(' · '));

      try { history.replaceState(null, '', '?user=' + encodeURIComponent(login)); } catch (e) { /* ignore */ }
    }).catch(function (err) {
      var msg;
      if (err && err.message === 'not-found') {
        msg = 'GitHub user not found — check the spelling and try again.';
      } else if (err && err.message === 'contributions-api') {
        msg = 'The contributions API is unreachable or rate-limited right now — try again in a moment, or explore the demo snapshots.';
      } else {
        msg = 'Could not hatch a dragon for @' + login + ' (' + ((err && err.message) || 'network error') + ').';
      }
      showError(msg);
      setStatus('');
    }).finally(function () {
      busy = false;
      $('#btn-generate').disabled = false;
      $('#dragon-card').classList.remove('is-loading');
    });
  }

  function loadDemo(slug) {
    var snapshot = DragonData.DEMO_SNAPSHOTS[slug];
    if (!snapshot) return;
    hideError();
    setStatus('');
    renderSnapshot(snapshot, { live: false });
    try { history.replaceState(null, '', '?demo=' + slug); } catch (e) { /* ignore */ }
    toast('Demo: ' + DragonRenderer.STAGE_NAMES[slug] + ' — type a username for the real thing');
  }

  /* ------------------------------------------------------------------ *
   * Stage carousel                                                      *
   * ------------------------------------------------------------------ */

  function paintCarouselCard(card) {
    var slug = card.getAttribute('data-stage');
    var canvas = card.querySelector('canvas');
    var ctx = canvas.getContext('2d');
    /* showcase dragons wear the canonical emerald coat — language tinting is
     * only ever shown for a real, searched username */
    var coat = null;
    var rig = DragonRenderer.rig(slug, coat);
    var frame = DragonRenderer.renderFrame(rig, 0);
    var img = ctx.createImageData(DragonRenderer.CANVAS_W, DragonRenderer.CANVAS_H);
    img.data.set(frame.data);
    ctx.putImageData(img, 0, 0);
  }

  function buildCarousel() {
    var grid = $('#stage-grid');
    DragonData.STAGES.forEach(function (s) {
      var card = document.createElement('article');
      card.className = 'stage-card';
      card.setAttribute('data-stage', s.slug);
      var range = s.hi === null ? fmt(s.lo) + '+'
        : (s.lo === s.hi ? String(s.lo) : fmt(s.lo) + ' – ' + fmt(s.hi));
      card.innerHTML =
        '<div class="stage-canvas-wrap"><canvas width="640" height="300"></canvas></div>' +
        '<div class="stage-card-body">' +
          '<div class="stage-card-head"><h3>' + s.name + '</h3><span class="stage-range">' + range + '</span></div>' +
          '<p>' + s.blurb + '</p>' +
          '<div class="stage-card-foot">' +
            (s.fires ? '<span class="chip chip-fire">🔥 fire breath</span>' : '<span class="chip chip-quiet">no fire yet</span>') +
            '<button class="btn btn-ghost btn-small" data-preview="' + s.slug + '">Preview</button>' +
          '</div>' +
        '</div>';
      grid.appendChild(card);
    });

    grid.addEventListener('click', function (ev) {
      var btn = ev.target.closest('[data-preview]');
      if (!btn) return;
      loadDemo(btn.getAttribute('data-preview'));
      document.getElementById('generator').scrollIntoView({ behavior: 'smooth' });
    });

    /* Paint all six cards eagerly — six 640×300 blits are nothing, and the
     * grid is usable (and screenshot-able) even before anything scrolls. */
    $$('.stage-card').forEach(function (c) { paintCarouselCard(c); });
  }

  /* ------------------------------------------------------------------ *
   * Ember particle background                                           *
   * ------------------------------------------------------------------ */

  function startEmbers() {
    var canvas = $('#embers');
    if (!canvas || !canvas.getContext) return;
    var ctx = canvas.getContext('2d');
    var visible = true;
    var W = 0, H = 0;
    var particles = [];
    var COLORS = ['255,200,40', '255,130,20', '210,255,245', '126,231,135'];

    function resize() {
      W = canvas.width = canvas.offsetWidth || 1;
      H = canvas.height = canvas.offsetHeight || 1;
    }
    resize();
    window.addEventListener('resize', resize);

    for (var i = 0; i < 42; i++) {
      particles.push({
        x: Math.random(),
        y: Math.random(),
        vy: 0.00012 + Math.random() * 0.00035,
        sway: Math.random() * Math.PI * 2,
        swaySpeed: 0.4 + Math.random(),
        size: Math.random() < 0.75 ? 1 : 2,
        color: COLORS[(Math.random() * COLORS.length) | 0],
        alpha: 0.25 + Math.random() * 0.55,
      });
    }

    if ('IntersectionObserver' in window) {
      new IntersectionObserver(function (en) { visible = en[0].isIntersecting; }).observe(canvas);
    }

    function draw(now) {
      ctx.clearRect(0, 0, W, H);
      for (var i = 0; i < particles.length; i++) {
        var p = particles[i];
        p.y -= p.vy;
        if (p.y < -0.05) { p.y = 1.05; p.x = Math.random(); }
        var x = (p.x + 0.012 * Math.sin(p.sway + now * 0.0004 * p.swaySpeed)) * W;
        var y = p.y * H;
        ctx.fillStyle = 'rgba(' + p.color + ',' + p.alpha + ')';
        ctx.fillRect(x | 0, y | 0, p.size, p.size);
      }
    }
    function tick(now) {
      requestAnimationFrame(tick);
      if (!visible || document.hidden) return;
      draw(now);
    }
    draw(0); // first frame without waiting for rAF
    requestAnimationFrame(tick);
  }

  /* ------------------------------------------------------------------ *
   * Wiring                                                              *
   * ------------------------------------------------------------------ */

  function wire() {
    mainCanvas = $('#dragon-canvas');
    mainCtx = mainCanvas ? mainCanvas.getContext('2d') : null;

    $('#gen-form').addEventListener('submit', function (ev) {
      ev.preventDefault();
      generate($('#gen-input').value);
    });

    $$('[data-user]').forEach(function (btn) {
      btn.addEventListener('click', function () { generate(btn.getAttribute('data-user')); });
    });

    $$('[data-demo]').forEach(function (btn) {
      btn.addEventListener('click', function () { loadDemo(btn.getAttribute('data-demo')); });
    });

    $('#btn-download').addEventListener('click', function () {
      if (!state.rig) return;
      downloadCanvas(DragonRenderer.stillCanvas(state.rig),
        'git-dragon-' + (state.live ? state.snapshot.login : 'demo') + '-' + state.stage + '.png');
      toast('PNG still downloaded');
    });

    $('#btn-share-card').addEventListener('click', function () {
      if (!state.rig) return;
      downloadCanvas(DragonRenderer.shareCardCanvas(state.rig),
        'git-dragon-' + (state.live ? state.snapshot.login : 'demo') + '-card.png');
      toast('Share card downloaded (1280×640)');
    });

    $('#btn-copy-link').addEventListener('click', function () {
      copyText(location.href, 'Page link copied');
    });

    $('#btn-copy-embed').addEventListener('click', function () {
      copyText($('#embed-link').textContent, 'Markdown copied');
    });

    $('#btn-copy-action').addEventListener('click', function () {
      copyText($('#embed-action').textContent, 'Workflow copied');
    });

    $('#btn-random').addEventListener('click', function () {
      /* random offline demo stage — no account needed */
      var stages = Object.keys(DragonData.DEMO_SNAPSHOTS);
      loadDemo(stages[(Math.random() * stages.length) | 0]);
    });

    startEmbers();
  }

  /* ------------------------------------------------------------------ *
   * Boot                                                                *
   * ------------------------------------------------------------------ */

  function boot() {
    wire();
    setStatus('Loading sprites…');
    DragonRenderer.preload(function (done, total) {
      setStatus('Loading sprites… ' + done + '/' + total);
    }).then(function () {
      setStatus('');
      buildCarousel(); // sprites are loaded now — eager paint is safe
      var params = new URLSearchParams(location.search);
      var user = params.get('user');
      var demo = params.get('demo');
      if (user && normalizeLogin(user)) {
        generate(user);
      } else if (demo && DragonData.DEMO_SNAPSHOTS[demo]) {
        loadDemo(demo);
      } else {
        loadDemo('legendary');
      }
    }).catch(function (err) {
      setStatus('');
      showError('Sprite assets failed to load (' + ((err && err.message) || err) + '). Check your connection and reload.');
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot);
  } else {
    boot();
  }
})();
