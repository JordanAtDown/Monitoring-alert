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

/** Opens the real page in a simulated browser, with the API answering from recorded responses. */
async function openPage({ update = UPDATE_NONE, status = fixture("status.json"), summary = fixture("summary-idle.json") } = {}) {
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
    const body = { "/api/status": status, "/api/update": update, "/api/summary": summary, "/api/trend": fixture("trend-cpu-idle.json") }[pathname];
    return body ? new Response(JSON.stringify(body)) : new Response("{}", { status: 404 });
  };
  w.eval(code);
  for (let i = 0; i < 120 && !w.document.querySelector("#detail .story"); i++) await new Promise((r) => setTimeout(r, 25));
  const d = w.document;
  const text = (sel) => [...d.querySelectorAll(sel)].map((e) => e.textContent.replace(/\s+/g, " ").trim());
  return { w, d, text, errors, close: () => w.close() };
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

test("a collector that cannot reach LibreHardwareMonitor says what to do", async () => {
  const status = fixture("status.json");
  status.collector.consecutive_failures = 4;
  status.collector.last_error = "Cannot reach LibreHardwareMonitor";
  const page = await openPage({ status });
  assert.match(page.text("#status")[0], /Les températures ne sont plus relevées depuis 2 min\./);
  assert.match(page.text("#status")[0], /lancez-le en administrateur.*Remote Web Server/);
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
