import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { JSDOM } from "jsdom";
import { bundleApp } from "./bundle.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = (name) => JSON.parse(readFileSync(path.join(here, "fixtures", name), "utf8"));
const code = await bundleApp();

const RELEASE = {
  version: "2.1.0",
  page_url: "https://github.com/JordanAtDown/Monitoring-alert/releases/tag/v2.1.0",
  download_url: "https://github.com/JordanAtDown/Monitoring-alert/releases/download/v2.1.0/monitoring_2.1.0.exe",
  notes: "Un tableau de bord plus lisible.",
  published_at: "2026-11-20T10:00:00Z",
};
const UPDATE_AVAILABLE = { current: "2.0.0", enabled: true, available: true, checked_at: "2026-11-11T15:00:00", error: null, latest: RELEASE };
const UPDATE_NONE = { current: "2.0.0", enabled: true, available: false, checked_at: "2026-11-11T15:00:00", error: null, latest: { ...RELEASE, version: "2.0.0" } };

const check = (id, title, level, message) => ({ id, title, level, message });
const HEALTH_OK = {
  overall: "ok",
  summary: "Tout fonctionne",
  checks: [
    check("collection", "Relevé des températures", "ok", "Dernière mesure il y a 2 min (toutes les 5 min)."),
    check("sensors", "Capteurs", "ok", "Les 15 capteurs répondent."),
    check("database", "Base de données", "ok", "Lisible, 11,6 Mo et 6654 relevés, dernière écriture il y a 2 min."),
    check("maintenance", "Résumés quotidiens", "ok", "À jour (dernier calcul à 14:30)."),
    check("autostart", "Démarrage automatique", "ok", "Le programme redémarre tout seul à l'ouverture de session."),
    check("updates", "Mises à jour", "ok", "Vous avez la dernière version (2.0.0)."),
  ],
};

/** Opens the real page in a simulated browser, with the API answering from recorded responses. */
const READY = { home: ".comp-card", curves: "#curves .picker", details: "#components table", status: "#health" };

async function openPage({ page = "home", hash = "", update = UPDATE_NONE, status = fixture("status.json"), summary = fixture("summary-idle.json"), health = HEALTH_OK } = {}) {
  const dom = new JSDOM('<!doctype html><html><body><div id="app"></div></body></html>', {
    url: `http://127.0.0.1:8787/#page=${page}${hash}`,
    runScripts: "outside-only",
    pretendToBeVisual: true,
  });
  const w = dom.window;
  w.matchMedia = () => ({ matches: false, addEventListener() {}, removeEventListener() {} });
  w.ResizeObserver = class { observe() {} disconnect() {} };
  w.HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  w.HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
  w.Element.prototype.scrollIntoView = function () {};
  const errors = [];
  const requests = [];
  w.addEventListener("error", (e) => errors.push(String(e.message)));
  w.fetch = async (url) => {
    const u = new URL(url, "http://x");
    requests.push(u.pathname + u.search);
    const body = { "/api/status": status, "/api/health": health, "/api/update": update, "/api/summary": summary, "/api/trend": fixture("trend-cpu-idle.json") }[u.pathname];
    return body ? new Response(JSON.stringify(body)) : new Response("{}", { status: 404 });
  };
  w.eval(code);
  const d = w.document;
  for (let i = 0; i < 120 && !d.querySelector(READY[page]); i++) await new Promise((r) => setTimeout(r, 25));
  const text = (sel) => [...d.querySelectorAll(sel)].map((e) => e.textContent.replace(/\s+/g, " ").trim());
  const settle = () => new Promise((r) => setTimeout(r, 120));
  return { w, d, text, title: () => d.title, errors, requests, settle, close: () => w.close() };
}

const click = (page, el) => el.dispatchEvent(new page.w.Event("click", { bubbles: true }));
const change = (page, el) => el.dispatchEvent(new page.w.Event("change", { bubbles: true }));

// ── Navigation ───────────────────────────────────────────────────────────────

test("a menu leads to four pages, and the current one is marked", async () => {
  const page = await openPage({ page: "curves" });
  assert.deepEqual(page.text("#tabs a"), ["Accueil", "Courbes", "Détails", "État"]);
  assert.equal(page.d.querySelector('#tabs [aria-current="page"]').textContent, "Courbes");
  assert.equal(page.d.querySelector(".subtitle"), null, "no long question under the title any more");
  assert.deepEqual(page.errors, []);
  page.close();
});

// ── Home ─────────────────────────────────────────────────────────────────────

test("the home page says in plain words how many components got warmer, stayed stable or cooled", async () => {
  const page = await openPage();
  const verdict = page.text("#verdict")[0];
  assert.match(verdict, /Depuis le 17 septembre 2025 \(environ 7 mois de suivi\), quand le PC est au repos/);
  assert.match(verdict, /3composants plus chauds qu'au début/);
  assert.match(verdict, /3composants stables/);
  assert.match(verdict, /1composant plus froid qu'au début/);
  assert.match(verdict, /Le changement le plus marqué : Carte graphique \(−2,6 °C\)/);
  assert.deepEqual(page.errors, []);
  page.close();
});

test("each part of the PC has its own card: the case and the motherboard apart, the disks numbered", async () => {
  const page = await openPage();
  assert.deepEqual(page.text(".comp-title"), ["Processeur", "Carte graphique", "Carte mère", "Boîtier", "Disque 1", "Disque 2", "Disque 3"]);
  const subtitles = page.text(".comp-sub");
  assert.equal(subtitles[0], "AMD Ryzen 7 5700X", "the motherboard's processor probe does not add a model");
  assert.equal(subtitles[2], "Gigabyte B550I AORUS PRO AX");
  assert.equal(subtitles[3], "", "the case has no model");
  assert.deepEqual(subtitles.slice(4), ["CT500P310SSD8", "Samsung SSD 860 QVO 1TB", "Samsung SSD 970 EVO Plus 250GB"]);
  page.close();
});

test("the little curves take the color of the trend, and the word stays next to them", async () => {
  const page = await openPage();
  const stroke = (title) => [...page.d.querySelectorAll(".comp-card")].find((c) => c.querySelector(".comp-title").textContent === title).querySelector("polyline").getAttribute("stroke");
  assert.equal(stroke("Boîtier"), "var(--diverge-warm)");
  assert.equal(stroke("Carte graphique"), "var(--diverge-cool)");
  assert.equal(stroke("Processeur"), "var(--ink-muted)");
  assert.match(page.text(".comp-card")[3], /Boîtier.*Plus chaud/);
  page.close();
});

test("the filter 'Le PC est…' sits in the cards it changes, not in a bar at the top", async () => {
  const page = await openPage();
  assert.equal(page.d.querySelectorAll("#verdict .segmented").length, 1);
  assert.equal(page.d.querySelectorAll("header .segmented, #tabs .segmented").length, 0);
  click(page, [...page.d.querySelectorAll("#verdict .segment")].find((b) => b.textContent === "Charge légère"));
  await page.settle();
  assert.ok(page.requests.includes("/api/summary?load=light"));
  assert.match(page.text("#verdict")[0], /quand le PC est peu sollicité/);
  page.close();
});

test("a card of the home page opens the curves with that component alone", async () => {
  const page = await openPage();
  const card = [...page.d.querySelectorAll(".comp-card")].find((c) => c.querySelector(".comp-title").textContent === "Boîtier");
  assert.match(card.getAttribute("href"), /page=curves/);
  assert.match(card.getAttribute("href"), /only=case/);
  page.close();
  const curves = await openPage({ page: "curves", hash: "&only=case" });
  const checked = [...curves.d.querySelectorAll(".picker input")].map((i) => i.checked);
  assert.deepEqual(checked, [false, false, false, true, false, false, false]);
  assert.equal(curves.d.querySelector(".u-wrap-stub").getAttribute("data-labels"), "Boîtier|Au début : 37,7 °C", "one line, and the level of the beginning");
  curves.close();
});

test("too little history says how much is needed instead of inventing a trend", async () => {
  const summary = fixture("summary-idle.json").map((s) => ({ ...s, sufficient: false, delta: null, baseline: null, days: 12, span_days: 20 }));
  const page = await openPage({ summary });
  assert.match(page.text("#verdict")[0], /Pas encore assez d'historique pour comparer.*60 jours de suivi dont 40 jours avec des mesures/);
  assert.match(page.text(".comp-card")[0], /Pas assez de recul/);
  page.close();
});

test("the technical vocabulary stays out of the main pages", async () => {
  for (const name of ["home", "details"]) {
    const page = await openPage({ page: name });
    const main = page.text("#page")[0].toLowerCase();
    for (const word of ["médiane", "p95", "pente", "baseline", "écart type"]) assert.ok(!main.includes(word), `"${word}" appears on ${name}`);
    page.close();
  }
});

// ── Curves ───────────────────────────────────────────────────────────────────

test("the curves show every component, each with its own color, and any can be switched off", async () => {
  const page = await openPage({ page: "curves" });
  assert.deepEqual(page.text(".series-name"), ["Processeur", "Carte graphique", "Carte mère", "Boîtier", "Disque 1", "Disque 2", "Disque 3"]);
  assert.ok([...page.d.querySelectorAll(".picker input")].every((i) => i.checked));
  assert.equal(page.d.querySelector(".u-wrap-stub").getAttribute("data-labels"), "Processeur|Carte graphique|Carte mère|Boîtier|Disque 1|Disque 2|Disque 3");

  const swatches = [...page.d.querySelectorAll(".picker .swatch")].map((s) => s.style.background);
  assert.equal(new Set(swatches).size, 7, "one color per component");

  const disk1 = page.d.querySelectorAll(".picker input")[4];
  disk1.checked = false;
  change(page, disk1);
  assert.equal(page.d.querySelector(".u-wrap-stub").getAttribute("data-labels"), "Processeur|Carte graphique|Carte mère|Boîtier|Disque 2|Disque 3");
  assert.deepEqual([...page.d.querySelectorAll(".picker .swatch")].map((s) => s.style.background), swatches, "switching one off never repaints the others");

  click(page, [...page.d.querySelectorAll(".link-button")].find((b) => b.textContent === "Tout masquer"));
  assert.ok(page.d.querySelector("#curves .u-wrap-stub") === null);
  assert.match(page.text("#curves")[0], /Aucun composant affiché/);
  click(page, [...page.d.querySelectorAll(".link-button")].find((b) => b.textContent === "Tout afficher"));
  assert.equal(page.d.querySelector(".u-wrap-stub").getAttribute("data-series").length > 0, true);
  assert.deepEqual(page.errors, []);
  page.close();
});

test("the curves can show the change since the beginning, which makes small drifts visible", async () => {
  const page = await openPage({ page: "curves" });
  assert.match(page.text(".series-now")[0], /47,4 °C/);
  click(page, [...page.d.querySelectorAll("#curves .segment")].find((b) => b.textContent === "Écart depuis le début"));
  assert.match(page.text(".series-now")[0], /−0,9 °C/);
  assert.match(page.d.querySelector(".u-wrap-stub").getAttribute("data-labels"), /Au début$/, "a line marks the beginning (0)");
  page.close();
});

test("the filter 'Le PC est…' is right above the chart", async () => {
  const page = await openPage({ page: "curves" });
  const card = page.d.querySelector("#curves");
  assert.ok(card.querySelector(".segmented"));
  const kids = [...card.children].map((c) => c.className);
  assert.ok(kids.indexOf("load-filter") < kids.indexOf("curve-controls"));
  page.close();
});

// ── Details ──────────────────────────────────────────────────────────────────

test("the details list every sensor under its component, the processor probe of the motherboard with the processor", async () => {
  const page = await openPage({ page: "details" });
  assert.deepEqual(page.text("#components caption"), [
    "Processeur — AMD Ryzen 7 5700X",
    "Carte graphique — NVIDIA GeForce RTX 5060",
    "Carte mère — Gigabyte B550I AORUS PRO AX",
    "Boîtier",
    "Disque 1 — CT500P310SSD8",
    "Disque 2 — Samsung SSD 860 QVO 1TB",
    "Disque 3 — Samsung SSD 970 EVO Plus 250GB",
  ]);
  const tables = [...page.d.querySelectorAll("#components table")];
  const names = (i) => [...tables[i].querySelectorAll("tbody th .sensor")].map((e) => e.textContent);
  assert.deepEqual(names(0), ["Cœurs du processeur", "Puce de calcul du processeur", "Sonde de la carte mère, près du processeur"]);
  assert.deepEqual([...names(2)].sort(), ["Alimentation du processeur", "Alimentation du processeur (SoC)", "Chipset de la carte mère"].sort());
  assert.ok(!names(2).includes("Sonde de la carte mère, près du processeur"));
  assert.deepEqual(names(3), ["Intérieur du boîtier"]);
  assert.deepEqual(names(6), ["Température du disque", "Sonde interne n° 1", "Sonde interne n° 2"]);
  page.close();
});

test("a sensor opens in place with its story, its curve and its figures", async () => {
  const page = await openPage({ page: "details" });
  const row = page.d.querySelector("#components tbody tr");
  click(page, row);
  await page.settle();
  assert.match(page.text(".detail .story")[0], /Au début \(du 17 septembre 2025 au 16 octobre 2025\), sa température habituelle était de 37,7 °C\. Ces 30 derniers jours : 42,3 °C, soit 4,7 °C de plus qu'au début\./);
  assert.match(page.text(".detail .advice")[0], /poussière.*pâte thermique.*ventilateur/);
  assert.equal(page.d.querySelector(".detail .u-wrap-stub").getAttribute("data-labels").split("|").length, 2, "one line plus the beginning");
  assert.equal(page.d.querySelectorAll(".detail .toggle").length, 0, "no extra layers to configure");
  assert.ok(page.d.querySelector('#components tr[aria-expanded="true"]'));
  click(page, page.d.querySelector("#components tbody tr"));
  assert.equal(page.d.querySelector(".detail"), null, "a second click closes it");
  page.close();
});

// ── Update ───────────────────────────────────────────────────────────────────

test("without a newer version there is no update button and the footer says so", async () => {
  const page = await openPage({ update: UPDATE_NONE });
  assert.equal(page.d.querySelector(".update-button"), null);
  assert.match(page.text("#footer")[0], /Monitoring 2\.0\.0 — à jour/);
  page.close();
});

test("a newer version shows a button that opens the steps to follow", async () => {
  const page = await openPage({ update: UPDATE_AVAILABLE });
  const button = page.d.querySelector(".update-button");
  assert.match(button.textContent, /Mise à jour disponible : version 2\.1\.0/);
  button.dispatchEvent(new page.w.Event("click", { bubbles: true }));

  const dialog = page.d.getElementById("update-dialog");
  assert.ok(dialog.hasAttribute("open"));
  const text = dialog.textContent.replace(/\s+/g, " ");
  assert.match(text, /Vous avez la version 2\.0\.0\. La version 2\.1\.0 est disponible/);
  assert.match(text, /Téléchargez le programme d'installation/);
  assert.match(text, /Lancez le fichier téléchargé.*monitoring_2\.1\.0\.exe/);
  assert.match(text, /Vos mesures et votre configuration sont conservées/);
  assert.deepEqual([...dialog.querySelectorAll("a")].map((a) => a.getAttribute("href")), [RELEASE.download_url, RELEASE.page_url]);
  assert.match(page.text("#footer")[0], /la version 2\.1\.0 est disponible/);
  page.close();
});

test("links and notes that do not come from GitHub are never followed or interpreted", async () => {
  const page = await openPage({
    update: { ...UPDATE_AVAILABLE, latest: { ...RELEASE, page_url: "https://evil.example/", download_url: "javascript:alert(1)", notes: "<img src=x onerror=alert(1)><script>alert(2)</script>" } },
  });
  page.d.querySelector(".update-button").dispatchEvent(new page.w.Event("click", { bubbles: true }));
  const dialog = page.d.getElementById("update-dialog");
  assert.equal(dialog.querySelectorAll("a").length, 0);
  assert.equal(dialog.querySelectorAll("img, script").length, 0);
  assert.match(dialog.textContent, /<img src=x onerror=alert\(1\)>/, "shown as text");
  page.close();
});

test("when everything works the page says so, and the tab and the header badge say it too", async () => {
  const page = await openPage({ page: "status" });
  assert.match(page.text("#health .status")[0], /✓Tout fonctionne/);
  assert.equal(page.title(), "✓ Monitoring");
  assert.equal(page.d.querySelectorAll("#health .problems").length, 0);
  assert.match(page.text(".pill")[0], /✓Tout fonctionne/, "the header badge says it on every page");
  assert.equal(page.d.querySelectorAll("#health .check").length, 6);
  assert.match(page.text("#health .check")[0], /Relevé des températures.*Dernière mesure il y a 2 min/);
  page.close();
});

test("a problem is stated plainly on the status page and in the header badge", async () => {
  const health = {
    overall: "bad",
    summary: "Un problème empêche le suivi",
    checks: [
      check("collection", "Relevé des températures", "bad", "Les températures ne sont plus relevées (dernière mesure il y a 20 min) : LibreHardwareMonitor ne répond pas."),
      ...HEALTH_OK.checks.slice(1),
    ],
  };
  const page = await openPage({ page: "status", health });
  assert.match(page.text("#health .status")[0], /✕Un problème empêche le suivi/);
  assert.match(page.text("#health .problems")[0], /Relevé des températures : Les températures ne sont plus relevées.*LibreHardwareMonitor ne répond pas/);
  // The page states what is wrong; it does not give orders.
  assert.ok(!page.text("#health")[0].includes("Que faire"));
  assert.match(page.text(".pill")[0], /✕Un problème empêche le suivi/);
  assert.equal(page.title(), "✕ Monitoring (1)");
  page.close();
});

test("warnings are counted in the tab title and never called problems", async () => {
  const health = {
    overall: "warn",
    summary: "2 points à vérifier",
    checks: [
      check("sensors", "Capteurs", "warn", "1 capteur ne répond plus : Core (GPU).", "Un disque en veille ou un composant retiré peut l'expliquer."),
      check("autostart", "Démarrage automatique", "warn", "Le programme ne redémarrera pas après un redémarrage du PC."),
      check("updates", "Mises à jour", "info", "La version 2.1.0 est disponible (vous avez la 2.0.0)."),
    ],
  };
  const page = await openPage({ page: "status", health });
  assert.match(page.text("#health .status")[0], /⚠2 points à vérifier/);
  assert.equal(page.title(), "⚠ Monitoring (2)");
  assert.equal(page.d.querySelectorAll("#health .problems li").length, 2, "information is not listed as a problem");
  assert.equal(page.d.querySelectorAll("#health .check-info").length, 1);
  page.close();
});

test("when the program cannot be reached the page says so instead of freezing on old figures", async () => {
  const page = await openPage({ page: "status", health: null });
  assert.match(page.text("#health")[0], /La page ne parvient plus à joindre le programme/);
  assert.equal(page.title(), "✕ Monitoring");
  page.close();
});


test("the status page says where the data is and for how long it is kept", async () => {
  const page = await openPage({ page: "status" });
  const [daily, raw, file] = page.text("#data li");
  assert.match(daily, /Résumé de chaque jour pour tous les capteurs, depuis le 17 septembre 2025 : conservé sans limite/);
  assert.match(raw, /une toutes les 5 minutes\) : conservées 365 jours/);
  assert.match(file, /E:\\monitoring\.db\\temperatures\.db/);
  page.close();
});
