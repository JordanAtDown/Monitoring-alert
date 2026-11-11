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

const check = (id, title, level, message, advice = null) => ({ id, title, level, message, advice });
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
async function openPage({ update = UPDATE_NONE, status = fixture("status.json"), summary = fixture("summary-idle.json"), health = HEALTH_OK } = {}) {
  const dom = new JSDOM('<!doctype html><html><body><div id="app"></div></body></html>', {
    url: "http://127.0.0.1:8787/",
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
  w.addEventListener("error", (e) => errors.push(String(e.message)));
  w.fetch = async (url) => {
    const { pathname } = new URL(url, "http://x");
    const body = { "/api/status": status, "/api/health": health, "/api/update": update, "/api/summary": summary, "/api/trend": fixture("trend-cpu-idle.json") }[pathname];
    return body ? new Response(JSON.stringify(body)) : new Response("{}", { status: 404 });
  };
  w.eval(code);
  for (let i = 0; i < 120 && !w.document.querySelector("#detail .story"); i++) await new Promise((r) => setTimeout(r, 25));
  const d = w.document;
  const title = () => d.title;
  const text = (sel) => [...d.querySelectorAll(sel)].map((e) => e.textContent.replace(/\s+/g, " ").trim());
  return { w, d, text, title, errors, close: () => w.close() };
}

test("the summary says in plain words how many components got warmer, stayed stable or cooled", async () => {
  const page = await openPage();
  const verdict = page.text("#verdict")[0];
  assert.match(verdict, /Depuis le 17 septembre 2025 \(environ 1,2 an de suivi\), quand le PC est au repos/);
  assert.match(verdict, /2composants plus chauds qu'au début/);
  assert.match(verdict, /1composant stable/);
  assert.match(verdict, /Le changement le plus marqué : Cœurs du processeur \(\+4,7 °C\)/);
  assert.deepEqual(page.errors, []);
  page.close();
});

test("components are grouped by hardware, with readable names and the technical name kept", async () => {
  const page = await openPage();
  assert.deepEqual(page.text("#components caption"), ["Processeur — AMD Ryzen 7 5700X", "Carte graphique — NVIDIA RTX 5060"]);
  const rows = page.text("#components tbody tr");
  assert.equal(rows.length, 3);
  assert.match(rows[0], /Cœurs du processeurCore \(Tctl\/Tdie\)37,7 °C42,3 °CPlus chaud\+4,7 °C/);
  assert.match(rows[2], /Cœur de la carte graphiqueGPU Core33,3 °C32,2 °CStable−1,1 °C/);
  page.close();
});

test("the technical vocabulary stays out of the main view", async () => {
  const page = await openPage();
  const main = page.text("#verdict")[0] + page.text("#components")[0] + page.text("#detail .story")[0];
  for (const word of ["médiane", "p95", "pente", "baseline", "écart type"]) {
    assert.ok(!main.toLowerCase().includes(word), `"${word}" appears in the main view`);
  }
  page.close();
});

test("the chosen component is explained in a sentence, with advice only when it got warmer", async () => {
  const page = await openPage();
  assert.match(page.text("#detail .story")[0], /Au début \(du 17 septembre 2025 au 16 octobre 2025\), sa température habituelle était de 37,7 °C\. Ces 30 derniers jours : 42,3 °C, soit 4,7 °C de plus qu'au début\./);
  assert.match(page.text("#detail .advice")[0], /poussière.*pâte thermique.*ventilateur/);
  page.close();
});

test("the chart offers its extra layers as options, off by default", async () => {
  const page = await openPage();
  assert.equal(page.d.querySelector(".u-wrap-stub").getAttribute("data-series"), "4");
  const [band, trendLine] = page.d.querySelectorAll("#detail .toggle input");
  assert.ok(!band.checked && !trendLine.checked);
  trendLine.checked = true;
  trendLine.dispatchEvent(new page.w.Event("change", { bubbles: true }));
  assert.equal(page.d.querySelector(".u-wrap-stub").getAttribute("data-series"), "5");
  page.close();
});

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

test("when everything works the page says so, and the tab says it too", async () => {
  const page = await openPage();
  assert.match(page.text("#health .status")[0], /✓Tout fonctionne/);
  assert.equal(page.title(), "✓ Monitoring");
  assert.equal(page.d.querySelectorAll("#health .problems").length, 0);
  // The detail is there but folded.
  const details = page.d.querySelector("#health details.checks");
  assert.ok(!details.hasAttribute("open"));
  assert.equal(page.d.querySelectorAll("#health .check").length, 6);
  assert.match(page.text("#health .check")[0], /Relevé des températures.*Dernière mesure il y a 2 min/);
  page.close();
});

test("a problem is shown with what to do, and the detail opens by itself", async () => {
  const health = {
    overall: "bad",
    summary: "Un problème empêche le suivi",
    checks: [
      check("collection", "Relevé des températures", "bad", "Les températures ne sont plus relevées (dernière mesure il y a 20 min) : LibreHardwareMonitor ne répond pas.", "Lancez LibreHardwareMonitor en administrateur et vérifiez que Options › Remote Web Server › Run est coché."),
      ...HEALTH_OK.checks.slice(1),
    ],
  };
  const page = await openPage({ health });
  assert.match(page.text("#health .status")[0], /✕Un problème empêche le suivi/);
  assert.match(page.text("#health .problems")[0], /Relevé des températures : Les températures ne sont plus relevées.*Que faire : Lancez LibreHardwareMonitor en administrateur/);
  assert.ok(page.d.querySelector("#health details.checks").hasAttribute("open"));
  assert.equal(page.title(), "✕ Monitoring (1)");
  page.close();
});

test("warnings are counted in the tab title and never called problems", async () => {
  const health = {
    overall: "warn",
    summary: "2 points à vérifier",
    checks: [
      check("sensors", "Capteurs", "warn", "1 capteur ne répond plus : Core (GPU).", "Un disque en veille ou un composant retiré peut l'expliquer."),
      check("autostart", "Démarrage automatique", "warn", "Le programme ne redémarrera pas après un redémarrage du PC.", "Relancez l'installateur."),
      check("updates", "Mises à jour", "info", "La version 2.1.0 est disponible (vous avez la 2.0.0)."),
    ],
  };
  const page = await openPage({ health });
  assert.match(page.text("#health .status")[0], /⚠2 points à vérifier/);
  assert.equal(page.title(), "⚠ Monitoring (2)");
  assert.equal(page.d.querySelectorAll("#health .problems li").length, 2, "information is not listed as a problem");
  assert.equal(page.d.querySelectorAll("#health .check-info").length, 1);
  page.close();
});

test("when the program cannot be reached the page says so instead of freezing on old figures", async () => {
  const page = await openPage({ health: null });
  assert.match(page.text("#health")[0], /La page ne parvient plus à joindre le programme/);
  assert.equal(page.title(), "✕ Monitoring");
  page.close();
});

test("too little history says how much is needed instead of inventing a trend", async () => {
  const summary = fixture("summary-idle.json").map((s) => ({ ...s, sufficient: false, delta: null, baseline: null, days: 12, span_days: 20 }));
  const page = await openPage({ summary });
  assert.match(page.text("#verdict")[0], /Pas encore assez d'historique pour comparer.*60 jours de suivi dont 40 jours avec des mesures/);
  assert.match(page.text("#components tbody tr")[0], /Pas assez de recul \(12 j de mesures\)/);
  page.close();
});

test("the page says where the data is and for how long it is kept", async () => {
  const page = await openPage();
  const [daily, raw, file] = page.text("#data li");
  assert.match(daily, /Résumé de chaque jour pour tous les capteurs, depuis le 17 septembre 2025 : conservé sans limite/);
  assert.match(raw, /une toutes les 5 minutes\) : conservées 365 jours/);
  assert.match(file, /E:\\monitoring\.db\\temperatures\.db/);
  page.close();
});

test("sensors are grouped by type of component, and several disks stay together under one title", async () => {
  const row = (hardware, sensor, delta) => ({ hardware, sensor, load_cat: "idle", days: 90, span_days: 200, sufficient: true, baseline: 40, current: 40 + delta, delta, slope_per_month: 0.1, sparkline: [40, 41] });
  const summary = [
    row("AMD Ryzen 7 5700X", "Core (Tctl/Tdie)", 0.5),
    row("Samsung SSD 970 EVO Plus 250GB", "Composite Temperature", 2.2),
    row("Samsung SSD 970 EVO Plus 250GB", "Temperature #2", 1.0),
    row("CT500P310SSD8", "Composite Temperature", 2.0),
    row("Samsung SSD 860 QVO 1TB ", "Temperature", 0.1),
    row("NVIDIA GeForce RTX 5060", "GPU Core", -1),
    row("Gigabyte B550I", "VRM MOS", 1),
  ];
  const page = await openPage({ summary });

  // One section per type, in a fixed, familiar order.
  assert.deepEqual(page.text("#components caption"), ["Processeur — AMD Ryzen 7 5700X", "Carte graphique — NVIDIA GeForce RTX 5060", "Carte mère — Gigabyte B550I", "Disques"]);

  // The three disks share the "Disques" table, each introduced by its own model, its sensors together.
  const disks = page.d.querySelectorAll("#components table")[3];
  const lines = [...disks.querySelectorAll("tbody tr")].map((tr) => tr.textContent.replace(/\s+/g, " ").trim().slice(0, 40));
  assert.match(lines[0], /^CT500P310SSD8/);
  assert.match(lines[1], /^Température du disque/);
  assert.match(lines[2], /^Samsung SSD 860 QVO 1TB/);
  assert.match(lines[4], /^Samsung SSD 970 EVO Plus 250GB/);
  assert.equal(disks.querySelectorAll("tr.subhead").length, 3);
  // A type with a single piece of hardware needs no sub-title.
  assert.equal(page.d.querySelectorAll("#components table")[0].querySelectorAll("tr.subhead").length, 0);
  page.close();
});
