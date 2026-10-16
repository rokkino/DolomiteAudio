//! DolomiteAudioRT control panel – compact tray flyout (served by `server.rs`).
//! Fully offline: system fonts only (Bahnschrift / Segoe UI Variable ship with Windows 10+).
//! The window title must start with "Dolomite": the driver uses it to know
//! that the user is still "in the DAW" while adjusting settings.

pub const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="it">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Dolomite Audio</title>
<style>
  :root {
    --bg: #202327;
    --card: #282c31;
    --well: #1a1d20;
    --line: #353a40;
    --ink: #eceae6;
    --muted: #9da3a9;
    --glow: #f2a1b5;          /* enrosadira – Dolomites alpenglow */
    --glow-dim: rgba(242,161,181,.14);
    --ok: #6ccb8a;
    --warn: #f2c14e;
    --hot: #ef5b5b;
    --blue: #7fb4e6;
    --num: "Bahnschrift", "DIN Alternate", "Segoe UI Variable Display", sans-serif;
    --txt: "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
  }
  * { box-sizing: border-box; margin: 0; }
  html, body { height: 100%; }
  body {
    background: var(--bg); color: var(--ink);
    font: 13px/1.4 var(--txt);
    display: flex; flex-direction: column;
    -webkit-user-select: none; user-select: none; overflow: hidden;
  }
  button, select { font: inherit; color: inherit; }
  :focus-visible { outline: 2px solid var(--glow); outline-offset: 1px; }

  /* ---------- top bar ---------- */
  .top { display: flex; align-items: center; gap: 8px; padding: 10px 10px 8px 14px; }
  .mark { width: 20px; height: 20px; flex: none; }
  .brand { font: 600 14px var(--num); letter-spacing: .02em; }
  .chip { margin-left: 4px; display: inline-flex; align-items: center; gap: 6px; padding: 2px 9px 2px 7px;
          border-radius: 99px; background: var(--card); color: var(--muted); font-size: 12px; white-space: nowrap; }
  .chip i { width: 7px; height: 7px; border-radius: 50%; background: #5a6067; }
  .chip.excl i { background: var(--ok); box-shadow: 0 0 6px var(--ok); }
  .chip.shared i { background: var(--blue); }
  .chip.bad i { background: var(--hot); box-shadow: 0 0 6px var(--hot); }
  .sp { flex: 1; }
  .icon { width: 28px; height: 28px; border: 0; border-radius: 6px; background: none; cursor: pointer;
          display: grid; place-items: center; color: var(--muted); }
  .icon:hover { background: var(--card); color: var(--ink); }
  .icon[aria-pressed="true"] { color: var(--glow); }
  .icon svg { width: 15px; height: 15px; }

  .scroll { flex: 1; overflow-y: auto; padding: 0 12px 12px; }
  .scroll::-webkit-scrollbar { width: 6px; }
  .scroll::-webkit-scrollbar-thumb { background: var(--line); border-radius: 3px; }

  .card { background: var(--card); border-radius: 10px; padding: 12px 14px; margin-bottom: 8px; }

  /* ---------- hero ---------- */
  .hero { display: flex; align-items: flex-end; gap: 14px; }
  .big { font: 300 46px/0.95 var(--num); font-variant-numeric: tabular-nums; letter-spacing: -.02em; }
  .big .u { font-size: 16px; color: var(--muted); margin-left: 3px; letter-spacing: 0; }
  .side { display: grid; gap: 1px; padding-bottom: 3px; color: var(--muted); font-size: 12px; }
  .side b { color: var(--ink); font: 500 14px var(--num); font-variant-numeric: tabular-nums; }
  .cap { color: var(--muted); font-size: 11.5px; margin-top: 3px; }
  .chain { display: flex; height: 6px; border-radius: 3px; overflow: hidden; margin: 10px 0 6px; background: var(--well); gap: 2px; }
  .chain span { height: 100%; transition: flex-grow .3s ease; border-radius: 2px; }
  .seg-buf { background: var(--glow); }
  .seg-q { background: #8a7fd6; }
  .seg-rs { background: #5fb0c9; }
  .legend { display: flex; gap: 12px; flex-wrap: wrap; color: var(--muted); font-size: 11.5px; }
  .legend i { display: inline-block; width: 7px; height: 7px; border-radius: 2px; margin-right: 5px; }
  .where { margin-top: 8px; font-size: 12px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .where b { color: var(--ink); font-weight: 500; }

  /* ---------- auto-exclusive status ---------- */
  .auto { display: flex; gap: 10px; align-items: flex-start; font-size: 12px; color: var(--muted); }
  .auto .dot { width: 8px; height: 8px; border-radius: 50%; margin-top: 4px; flex: none; background: #5a6067; }
  .auto.on .dot { background: var(--ok); }
  .auto.rel .dot { background: var(--blue); }
  .auto.busy .dot { background: var(--warn); }
  .auto b { color: var(--ink); font-weight: 600; display: block; font-size: 12.5px; }

  /* ---------- meters ---------- */
  .meters { display: grid; grid-template-columns: 38px 1fr 40px; gap: 5px 8px; align-items: center; }
  .meters .lbl { font: 500 11px var(--num); color: var(--muted); white-space: nowrap; }
  .bar { position: relative; height: 6px; border-radius: 3px; background: var(--well); overflow: hidden; }
  .bar .fill { position: absolute; inset: 0;
    background: linear-gradient(to right, var(--ok) 0%, var(--ok) 70%, var(--warn) 72%, var(--warn) 90%, var(--hot) 92%); }
  .bar .cover { position: absolute; top: 0; bottom: 0; right: 0; width: 100%; background: var(--well); }
  .bar .hold { position: absolute; top: 0; bottom: 0; width: 2px; background: var(--ink); opacity: 0; }
  .bar.clip { box-shadow: 0 0 0 1px var(--hot); }
  .db { font: 500 11.5px var(--num); font-variant-numeric: tabular-nums; text-align: right; color: var(--muted); }

  /* ---------- stats ---------- */
  .stats { display: grid; grid-auto-flow: column; grid-auto-columns: 1fr; gap: 6px; }
  .stat[hidden] { display: none; }
  .stat b, .stat span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .stat { background: var(--well); border-radius: 8px; padding: 7px 9px; }
  .stat b { display: block; font: 500 15px var(--num); font-variant-numeric: tabular-nums; }
  .stat span { color: var(--muted); font-size: 11px; }
  .stat.alert b { color: var(--hot); }

  /* ---------- settings (collapsible) ---------- */
  details { background: var(--card); border-radius: 10px; margin-bottom: 8px; }
  summary { list-style: none; cursor: pointer; padding: 10px 14px; display: flex; align-items: center; gap: 8px; }
  summary::-webkit-details-marker { display: none; }
  summary .t { font-weight: 600; }
  summary .v { margin-left: auto; color: var(--muted); font-size: 12px; max-width: 190px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  summary::after { content: ""; width: 7px; height: 7px; border-right: 1.5px solid var(--muted); border-bottom: 1.5px solid var(--muted);
                   transform: rotate(-45deg); margin-left: 4px; transition: transform .15s; flex: none; }
  details[open] summary::after { transform: rotate(45deg); }
  .body { padding: 0 14px 12px; display: grid; gap: 10px; }
  select { width: 100%; background: var(--well); border: 1px solid var(--line); border-radius: 7px; padding: 7px 8px; }
  .seg { display: flex; background: var(--well); border-radius: 7px; padding: 2px; gap: 2px; }
  .seg button { flex: 1; padding: 6px 0; border: 0; border-radius: 5px; background: none; cursor: pointer;
                font-family: var(--num); font-variant-numeric: tabular-nums; color: var(--muted); font-size: 12.5px; }
  .seg button:hover { color: var(--ink); }
  .seg button[aria-pressed="true"] { background: var(--glow); color: #2a1b20; font-weight: 600; }
  .seg button.rec { box-shadow: inset 0 -2px 0 var(--ok); }
  .hint { color: var(--muted); font-size: 11.5px; }
  .row { display: flex; align-items: center; gap: 10px; cursor: pointer; }
  .row span { flex: 1; }
  .tog { appearance: none; width: 32px; height: 18px; border-radius: 9px; background: #41474e; position: relative; cursor: pointer; flex: none; }
  .tog::after { content: ""; position: absolute; top: 3px; left: 3px; width: 12px; height: 12px; border-radius: 50%; background: var(--ink); transition: transform .15s; }
  .tog:checked { background: var(--glow); }
  .tog:checked::after { transform: translateX(14px); background: #2a1b20; }
  .linkbtn { border: 0; background: none; color: var(--glow); cursor: pointer; padding: 0; font-size: 11.5px; }

  .foot { display: flex; gap: 8px; padding: 8px 12px 12px; border-top: 1px solid var(--line); }
  .btn { flex: 1; background: var(--card); border: 1px solid var(--line); border-radius: 7px; padding: 7px 10px; cursor: pointer; }
  .btn:hover { border-color: var(--glow); }

  .toast { position: fixed; left: 50%; bottom: 58px; transform: translate(-50%, 10px); opacity: 0; pointer-events: none;
           background: var(--ink); color: var(--bg); padding: 6px 12px; border-radius: 7px; transition: all .2s; font-size: 12px; white-space: nowrap; }
  .toast.show { opacity: 1; transform: translate(-50%, 0); }

  .idle .live { opacity: .35; }
  .empty { display: none; font-size: 12px; color: var(--muted); margin-top: 8px; }
  .idle .empty { display: block; }
  @media (prefers-reduced-motion: reduce) { * { transition: none !important; } }
</style>
</head>
<body class="idle">
<div class="top">
  <svg class="mark" viewBox="0 0 34 34" aria-hidden="true">
    <path d="M2 30 L11 12 L15 19 L21 6 L32 30 Z" fill="#f2a1b5"/>
    <path d="M2 30 L11 12 L15 19 L13 30 Z" fill="#c97f93"/>
  </svg>
  <span class="brand">Dolomite Audio</span>
  <span class="chip" id="chip"><i></i><span id="chipText">Inattivo</span></span>
  <span class="sp"></span>
  <button class="icon" id="pinBtn" aria-pressed="false" title="Tieni aperto">
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M6 2h4l-.5 4 2.5 2.5H4L6.5 6 6 2zM8 8.5V14"/></svg>
  </button>
  <button class="icon" id="closeBtn" title="Chiudi (resta nell'area di notifica)">
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 4l8 8M12 4l-8 8"/></svg>
  </button>
</div>

<div class="scroll">
  <section class="card">
    <div class="live">
      <div class="hero">
        <div>
          <div class="big" id="outMs">–<span class="u">ms</span></div>
          <div class="cap">Latenza uscita</div>
        </div>
        <div class="side">
          <span>Ingresso <b id="inMs">–</b></span>
          <span>Andata/ritorno <b id="rtMs">–</b></span>
        </div>
      </div>
      <div class="chain" aria-hidden="true">
        <span class="seg-buf" id="segBuf" style="flex-grow:1"></span>
        <span class="seg-q" id="segQ" style="flex-grow:1"></span>
        <span class="seg-rs" id="segRs" style="flex-grow:0"></span>
      </div>
      <div class="legend">
        <span><i style="background:var(--glow)"></i>DAW <b id="lgBuf">–</b></span>
        <span><i style="background:#8a7fd6"></i>Scheda <b id="lgQ">–</b></span>
        <span id="lgRsWrap" hidden><i style="background:#5fb0c9"></i>Conv. <b id="lgRs">–</b></span>
      </div>
      <div class="where" id="where">–</div>
    </div>
    <p class="empty">Nessun programma usa il driver. Nelle impostazioni audio della tua DAW scegli <b>DolomiteAudioRT</b> come dispositivo ASIO.</p>
  </section>

  <section class="card auto" id="auto" hidden>
    <span class="dot"></span>
    <div><b id="autoTitle">–</b><span id="autoText"></span></div>
  </section>

  <section class="card live">
    <div class="meters">
      <span class="lbl">OUT L</span><div class="bar" id="m0"><div class="fill"></div><div class="cover"></div><div class="hold"></div></div><span class="db" id="d0">-∞</span>
      <span class="lbl">OUT R</span><div class="bar" id="m1"><div class="fill"></div><div class="cover"></div><div class="hold"></div></div><span class="db" id="d1">-∞</span>
      <span class="lbl">IN L</span><div class="bar" id="m2"><div class="fill"></div><div class="cover"></div><div class="hold"></div></div><span class="db" id="d2">-∞</span>
      <span class="lbl">IN R</span><div class="bar" id="m3"><div class="fill"></div><div class="cover"></div><div class="hold"></div></div><span class="db" id="d3">-∞</span>
    </div>
  </section>

  <section class="stats live" style="margin-bottom:8px">
    <div class="stat"><b id="stLoad">–</b><span>Carico</span></div>
    <div class="stat" id="stUnderWrap"><b id="stUnder">0</b><span>Dropout</span></div>
    <div class="stat"><b id="stRate">–</b><span id="stRateCap">Frequenza</span></div>
    <div class="stat" id="stSyncWrap" hidden title="Ingresso e uscita hanno orologi diversi: il driver adatta di continuo la conversione, senza scartare campioni."><b id="stSync">–</b><span id="stSyncCap">Sync in</span></div>
  </section>

  <details id="dEngine">
    <summary><span class="t">Motore</span><span class="v" id="vEngine"></span></summary>
    <div class="body">
      <div class="seg" id="engSeg"></div>
      <p class="hint" id="engHint"></p>
      <label class="row" id="autoRow"><span>Rilascia l'esclusiva quando cambi programma o un'altra app suona</span><input type="checkbox" class="tog" id="autoTog"></label>
    </div>
  </details>

  <details id="dBuf">
    <summary><span class="t">Buffer</span><span class="v" id="vBuf"></span></summary>
    <div class="body">
      <div class="seg" id="bufSeg"></div>
      <p class="hint" id="bufHint">La DAW può cambiarlo nelle sue impostazioni audio.</p>
    </div>
  </details>

  <details id="dOut">
    <summary><span class="t">Uscita</span><span class="v" id="vOut"></span></summary>
    <div class="body"><select id="outSel" aria-label="Dispositivo di uscita"></select></div>
  </details>

  <details id="dIn">
    <summary><span class="t">Ingresso</span><span class="v" id="vIn"></span></summary>
    <div class="body">
      <label class="row"><span>Registra dagli ingressi</span><input type="checkbox" class="tog" id="inTog"></label>
      <select id="inSel" aria-label="Dispositivo di ingresso"></select>
    </div>
  </details>

  <details id="dRate">
    <summary><span class="t">Frequenza predefinita</span><span class="v" id="vRate"></span></summary>
    <div class="body">
      <div class="seg" id="rateSeg"></div>
      <p class="hint">Uguale a quella della scheda = nessuna conversione (bit-perfect).</p>
    </div>
  </details>
</div>

<div class="foot">
  <button class="btn" id="testBtn">Suono di prova</button>
</div>
<div class="toast" id="toast" role="status"></div>

<script>
const $ = id => document.getElementById(id);
const BUFFERS = [32, 64, 128, 256, 512, 1024];
const RATES = [44100, 48000, 88200, 96000];
const ENGINES = {
  shared: { label: 'Compatibile', hint: 'Mixer standard di Windows (10 ms). Usalo se le altre modalità gracchiano.' },
  low: { label: 'Bassa latenza', hint: 'Periodo minimo del mixer di Windows. Le altre app continuano a suonare.' },
  exclusive: { label: 'Esclusivo', hint: 'La scheda lavora solo per la DAW: latenza minima, anche in ingresso.' },
};
const AUTO = {
  1: ['on', 'Esclusivo attivo', 'Si rilascia appena passi a un altro programma.'],
  2: ['rel', 'Esclusiva rilasciata', 'La DAW è in secondo piano: le altre app possono suonare. Torna esclusivo quando rientri.'],
  3: ['rel', 'Condiviso: un\'altra app sta suonando', 'Torna esclusivo quando resta solo la DAW.'],
  4: ['busy', 'Scheda occupata', 'Un altro programma la usa in esclusiva: nuovo tentativo tra poco.'],
};
let cfg = {}, devs = { outputs: [], inputs: [] }, last = null;

function toast(msg) {
  const t = $('toast'); t.textContent = msg; t.classList.add('show');
  clearTimeout(toast._t); toast._t = setTimeout(() => t.classList.remove('show'), 1600);
}
const post = (url, body) => fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body || {}) });
async function save(patch, msg) {
  Object.assign(cfg, patch);
  await post('/api/config', cfg);
  renderSettings();
  if (msg) toast(msg);
}

function segmented(el, values, current, fmt, onPick, rec) {
  el.innerHTML = '';
  for (const v of values) {
    const b = document.createElement('button');
    b.textContent = fmt(v);
    b.setAttribute('aria-pressed', String(v === current));
    if (rec !== undefined && v === rec) { b.classList.add('rec'); b.title = 'Consigliato per questa scheda'; }
    b.onclick = () => onPick(v);
    el.appendChild(b);
  }
}
function fillSelect(sel, list, current, defName) {
  sel.innerHTML = '';
  for (const d of [{ id: '', name: defName }, ...list]) {
    const o = document.createElement('option');
    o.value = d.id; o.textContent = d.name; o.selected = (current || '') === d.id;
    sel.appendChild(o);
  }
}
const nameOf = (list, id, def) => (list.find(d => d.id === id) || { name: def }).name;

function recommendedBuffer() {
  if (!last || !last.alive || !last.period) return undefined;
  const need = last.period * last.daw_rate / (last.device_rate || last.daw_rate);
  return BUFFERS.find(b => b >= need) || BUFFERS[BUFFERS.length - 1];
}

function renderSettings() {
  const eng = cfg.exclusive ? 'exclusive' : (cfg.low_latency ? 'low' : 'shared');
  segmented($('engSeg'), ['shared', 'low', 'exclusive'], eng, v => ENGINES[v].label, v =>
    save({ low_latency: v !== 'shared', exclusive: v === 'exclusive' }, 'Motore: ' + ENGINES[v].label));
  $('engHint').textContent = ENGINES[eng].hint;
  $('vEngine').textContent = ENGINES[eng].label + (cfg.exclusive && cfg.auto_release ? ' · auto' : '');
  $('autoRow').hidden = !cfg.exclusive;
  $('autoTog').checked = !!cfg.auto_release;

  const rec = recommendedBuffer();
  segmented($('bufSeg'), BUFFERS, cfg.buffer_size, v => v, v => save({ buffer_size: v }, 'Buffer preferito: ' + v), rec);
  $('vBuf').textContent = cfg.buffer_size + ' campioni';
  $('bufHint').textContent = rec
    ? `Consigliato ${rec} (periodo della scheda ${last.period}). La DAW può cambiarlo nelle sue impostazioni audio.`
    : 'La DAW può cambiarlo nelle sue impostazioni audio.';

  fillSelect($('outSel'), devs.outputs, cfg.output_device, 'Predefinito di Windows');
  $('vOut').textContent = nameOf(devs.outputs, cfg.output_device, 'Predefinito di Windows');
  fillSelect($('inSel'), devs.inputs, cfg.input_device, 'Predefinito di Windows');
  $('inSel').disabled = !cfg.input_enabled;
  $('inTog').checked = !!cfg.input_enabled;
  $('vIn').textContent = cfg.input_enabled ? nameOf(devs.inputs, cfg.input_device, 'Predefinito di Windows') : 'Spento';

  segmented($('rateSeg'), RATES, cfg.sample_rate, v => (v / 1000) + 'k', v => save({ sample_rate: v }, 'Frequenza: ' + v + ' Hz'));
  $('vRate').textContent = (cfg.sample_rate / 1000) + ' kHz';
}

$('outSel').onchange = e => save({ output_device: e.target.value }, 'Uscita cambiata');
$('inSel').onchange = e => save({ input_device: e.target.value }, 'Ingresso cambiato');
$('inTog').onchange = e => save({ input_enabled: e.target.checked }, e.target.checked ? 'Ingressi attivi' : 'Ingressi spenti');
$('autoTog').onchange = e => save({ auto_release: e.target.checked }, e.target.checked ? 'Rilascio automatico attivo' : 'Esclusiva fissa');
$('testBtn').onclick = async () => { await post('/api/test_sound'); toast('Suono di prova'); };
$('closeBtn').onclick = () => post('/api/hide');
$('pinBtn').onclick = () => {
  const on = $('pinBtn').getAttribute('aria-pressed') !== 'true';
  $('pinBtn').setAttribute('aria-pressed', String(on));
  post('/api/pin', { pinned: on });
  toast(on ? 'Il pannello resta aperto' : 'Si chiude quando clicchi fuori');
};
document.addEventListener('keydown', e => { if (e.key === 'Escape') post('/api/hide'); });

// remember which sections are open
for (const d of document.querySelectorAll('details')) {
  try { d.open = localStorage.getItem('open_' + d.id) === '1'; } catch (_) {}
  d.addEventListener('toggle', () => { try { localStorage.setItem('open_' + d.id, d.open ? '1' : '0'); } catch (_) {} });
}

// ---------- meters ----------
const DB_MIN = -60;
const dbToFrac = db => Math.max(0, Math.min(1, (db - DB_MIN) / -DB_MIN));
const lin2db = v => v > 1e-6 ? 20 * Math.log10(v) : -Infinity;
const mst = [0, 1, 2, 3].map(() => ({ shown: 0, hold: -Infinity, holdT: 0, clip: 0, clipT: -Infinity }));
function drawMeter(i, peak, clipCount) {
  const m = $('m' + i), st = mst[i], now = performance.now();
  const db = lin2db(peak), target = dbToFrac(db);
  st.shown = target > st.shown ? target : Math.max(target, st.shown - 0.012);
  m.querySelector('.cover').style.width = ((1 - st.shown) * 100) + '%';
  if (db > st.hold || now - st.holdT > 1500) { st.hold = db; st.holdT = now; }
  const h = m.querySelector('.hold');
  h.style.left = `calc(${dbToFrac(st.hold) * 100}% - 2px)`;
  h.style.opacity = st.hold > DB_MIN ? 1 : 0;
  if (clipCount > st.clip) { st.clip = clipCount; st.clipT = now; }
  m.classList.toggle('clip', now - st.clipT < 2000);
  $('d' + i).textContent = isFinite(st.hold) && st.hold > -90 ? st.hold.toFixed(1) : '-∞';
}

// ---------- live telemetry ----------
const ms = (frames, rate) => rate ? (frames * 1000 / rate) : 0;
const fmtMs = v => v.toFixed(v < 10 ? 1 : 0);
const khz = r => (r / 1000).toString();
const MODES = ['Timer', 'Compatibile', 'Bassa latenza', 'Esclusivo'];

function render(s) {
  const alive = !!s.alive;
  document.body.classList.toggle('idle', !alive);
  const chip = $('chip');
  const dropping = alive && last && s.underruns > last.underruns;
  chip.className = 'chip' + (!alive ? '' : dropping ? ' bad' : s.mode === 3 ? ' excl' : ' shared');
  $('chipText').textContent = alive ? MODES[s.mode] || '–' : 'Inattivo';

  if (alive) {
    const rate = s.daw_rate;
    const rsFrames = s.rs_latency || 0;
    const qFrames = Math.max(0, s.out_latency - s.buffer - rsFrames);
    const outMs = ms(s.out_latency, rate), inMs = ms(s.in_latency, rate);
    $('outMs').firstChild.nodeValue = fmtMs(outMs);
    $('inMs').textContent = s.in_mode ? fmtMs(inMs) + ' ms' : '–';
    $('rtMs').textContent = s.in_mode ? fmtMs(outMs + inMs) + ' ms' : '–';
    $('segBuf').style.flexGrow = s.buffer; $('segQ').style.flexGrow = qFrames; $('segRs').style.flexGrow = rsFrames;
    $('lgBuf').textContent = `${s.buffer} · ${fmtMs(ms(s.buffer, rate))} ms`;
    $('lgQ').textContent = `${fmtMs(ms(qFrames, rate))} ms`;
    $('lgRsWrap').hidden = !s.resampling;
    $('lgRs').textContent = `${fmtMs(ms(rsFrames, rate))} ms`;
    const w = $('where');
    w.textContent = s.device + (s.in_mode ? '  ·  in: ' + s.in_device : '');
    w.title = w.textContent;

    $('stRate').textContent = s.resampling ? `${khz(s.daw_rate)}→${khz(s.device_rate)}` : `${khz(rate)} kHz`;
    $('stRateCap').textContent = s.resampling ? 'Convertita' : 'Bit-perfect';
    const load = s.load / 10;
    $('stLoad').textContent = load.toFixed(0) + ' %';
    $('stLoad').style.color = load > 80 ? 'var(--hot)' : load > 60 ? 'var(--warn)' : '';
    $('stUnder').textContent = s.underruns + (s.in_underruns ? ' / ' + s.in_underruns : '');
    $('stUnderWrap').classList.toggle('alert', s.underruns > 0);
    $('stSyncWrap').hidden = !(s.in_mode && s.adaptive);
    if (s.adaptive) {
      const p = s.drift_ppm;
      $('stSync').textContent = s.adaptive === 2 ? (p >= 0 ? '+' : '') + (Math.abs(p) < 10 ? p.toFixed(1) : p.toFixed(0)) + ' ppm' : 'aggancio…';
    }

    const a = AUTO[s.auto_state];
    $('auto').hidden = !a;
    if (a) {
      $('auto').className = 'card auto ' + a[0];
      $('autoTitle').textContent = a[1];
      $('autoText').textContent = a[2];
    }
    for (let i = 0; i < 4; i++) drawMeter(i, s.peak[i], s.clip[i]);
  } else {
    $('auto').hidden = true;
    for (let i = 0; i < 4; i++) drawMeter(i, 0, 0);
  }
  const hadPeriod = last && last.period;
  last = s;
  if (alive && !hadPeriod) renderSettings(); // show the recommended buffer
}

let es = null;
function connect() {
  if (es) return;
  es = new EventSource('/api/events');
  es.onmessage = e => { try { render(JSON.parse(e.data)); } catch (_) {} };
  es.onerror = () => { es.close(); es = null; setTimeout(() => { if (!document.hidden) connect(); }, 1000); };
}
async function refresh() {
  [cfg, devs] = await Promise.all([
    fetch('/api/config').then(r => r.json()),
    fetch('/api/devices').then(r => r.json()).catch(() => ({ outputs: [], inputs: [] })),
  ]);
  renderSettings();
}
// The tray hides the window instead of closing it: stop streaming while hidden.
document.addEventListener('visibilitychange', () => {
  if (document.hidden) { if (es) { es.close(); es = null; } }
  else { refresh(); connect(); }
});
fetch('/api/pin').then(r => r.json()).then(p => $('pinBtn').setAttribute('aria-pressed', String(!!p.pinned))).catch(() => {});
refresh().then(connect);
</script>
</body>
</html>
"##;
