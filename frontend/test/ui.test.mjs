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
const UPDATE_AVAILABLE = { current: "2.0.0", enabled: true, available: true, checked_at: "2026-11-11T15:00:00", error: null, latest: RELEASE, line: "Monitoring 2.0.0 — la version 2.1.0 est disponible", button: "Mise à jour disponible : version 2.1.0", published: "20 novembre 2026" };
const UPDATE_NONE = { current: "2.0.0", enabled: true, available: false, checked_at: "2026-11-11T15:00:00", error: null, latest: { ...RELEASE, version: "2.0.0" }, line: "Monitoring 2.0.0 — à jour", button: null, published: null };

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

async function openPage({ page = "home", hash = "", update = UPDATE_NONE, status = fixture("status.json"), overview = fixture("overview-idle.json"), curves = {}, sensor = fixture("sensor-disk1.json"), health = HEALTH_OK } = {}) {
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
  const curvesByMode = { temp: fixture("curves-idle-temp.json"), delta: fixture("curves-idle-delta.json"), ...curves };
  w.fetch = async (url) => {
    const u = new URL(url, "http://x");
    requests.push(u.pathname + u.search);
    const load = u.searchParams.get("load");
    const body = {
      "/api/status": status,
      "/api/health": health,
      "/api/update": update,
      "/api/overview": load === "idle" || !load ? overview : { ...overview, load: { ...overview.load, id: load, label: "Charge légère" }, headline: { ...overview.headline, lead: `Depuis le début, ${load} :` } },
      "/api/curves": curvesByMode[u.searchParams.get("mode") ?? "temp"],
      "/api/sensor": sensor,
    }[u.pathname];
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
const labels = (page) => page.d.querySelector(".u-wrap-stub").getAttribute("data-labels");

// ── Navigation ───────────────────────────────────────────────────────────────

test("a menu leads to four pages, and the current one is marked", async () => {
  const page = await openPage({ page: "curves" });
  assert.deepEqual(page.text("#tabs a"), ["Accueil", "Courbes", "Détails", "État"]);
  assert.equal(page.d.querySelector('#tabs [aria-current="page"]').textContent, "Courbes");
  assert.equal(page.d.querySelector(".subtitle"), null, "no long question under the title any more");
  assert.deepEqual(page.errors, []);
  page.close();
});

// ── The page decides nothing ─────────────────────────────────────────────────

test("the page prints what the backend says, word for word", async () => {
  const overview = fixture("overview-idle.json");
  overview.headline.lead = "Une phrase écrite par le backend :";
  overview.headline.tiles = [{ verdict: "warmer", count: 42, words: "mots du backend" }];
  overview.headline.detail = "Un détail du backend.";
  overview.components[0].title = "Mon composant";
  overview.components[0].main.verdict = { id: "warmer", label: "Un mot à moi", detail: "+9,9 °C" };
  overview.components[0].main.baseline = "1,0 °C";
  const page = await openPage({ overview });
  assert.match(page.text("#verdict")[0], /Une phrase écrite par le backend :.*42mots du backend.*Un détail du backend\./);
  assert.match(page.text(".comp-card")[0], /Mon composant.*Un mot à moi\+9,9 °C.*1,0 °C/);
  page.close();
});

// ── Home ─────────────────────────────────────────────────────────────────────

test("the home page gives the answer in a few words and counts the components", async () => {
  const page = await openPage();
  const verdict = page.text("#verdict")[0];
  assert.match(verdict, /Depuis le \d+ \w+ \d{4} \(environ .+ de suivi\), quand le PC est au repos :/);
  // Every component is counted once, under one word or another: the figures of the recording
  // day do not matter, that they add up does.
  const tiles = [...verdict.matchAll(/(\d+)composants? (?:plus chauds? qu'au début|stables?|plus froids? qu'au début|sans assez de recul)/g)];
  assert.ok(tiles.length >= 3, verdict);
  assert.equal(
    tiles.reduce((n, m) => n + Number(m[1]), 0),
    page.text(".comp-title").length,
  );
  assert.deepEqual(page.errors, []);
  page.close();
});

test("the home page says whether the room or one component is to blame", async () => {
  const page = await openPage();
  // The recorded backend weighs each component against the drift of the whole machine.
  assert.match(page.text("#verdict")[0], /que le reste de la machine \(qui a bougé de [+−]\d+,\d °C\)/);
  page.close();
});

test("a component that moved more than the rest of the machine says so on its card", async () => {
  const overview = fixture("overview-idle.json");
  overview.components[0].cause = { id: "alone", label: "Un mot du backend", detail: "9,9 °C de plus que le reste" };
  overview.components[1].cause = { id: "nothing", label: "Rien n'a bougé", detail: null };
  overview.components[2].cause = { id: "machine", label: "Comme toute la machine", detail: "+2,0 °C" };
  const page = await openPage({ overview });
  const cards = [...page.d.querySelectorAll(".comp-card")];
  assert.match(cards[0].textContent, /Un mot du backend9,9 °C de plus que le reste/);
  assert.ok(cards[0].querySelector(".cause-alone"), "the one that stands out is set apart");
  assert.equal(cards[1].querySelector(".comp-cause"), null, "nothing moved: no line of its own");
  assert.match(cards[2].textContent, /Comme toute la machine\+2,0 °C/);
  page.close();
});

test("each part of the PC has its own card: the case and the motherboard apart, the disks numbered", async () => {
  const page = await openPage();
  assert.deepEqual(page.text(".comp-title"), ["Processeur", "Carte graphique", "Carte mère", "Boîtier", "Disque 1", "Disque 2", "Disque 3"]);
  const subtitles = page.text(".comp-sub");
  assert.equal(subtitles[0], "AMD Ryzen 7 5700X");
  assert.equal(subtitles[3], "", "the case has no model");
  assert.deepEqual(subtitles.slice(4), ["CT500P310SSD8", "Samsung SSD 860 QVO 1TB", "Samsung SSD 970 EVO Plus 250GB"]);
  page.close();
});

test("the little curves take the color of the trend, and the word stays next to them", async () => {
  const colors = { warmer: "var(--diverge-warm)", cooler: "var(--diverge-cool)", stable: "var(--ink-muted)", unknown: "var(--ink-muted)" };
  const ids = Object.keys(colors);
  const overview = fixture("overview-idle.json");
  // The verdicts are driven here: the color must follow the word, whichever component carries it.
  ids.forEach((id, i) => {
    overview.components[i].main = { ...overview.components[i].main, verdict: { id, label: `Mot ${i}`, detail: null } };
  });
  const page = await openPage({ overview });
  const cards = [...page.d.querySelectorAll(".comp-card")];
  ids.forEach((id, i) => {
    assert.equal(cards[i].querySelector("polyline").getAttribute("stroke"), colors[id], id);
    assert.match(cards[i].textContent, new RegExp(`Mot ${i}`), "the word stays next to the curve");
  });
  page.close();
});

test("the filter 'Le PC est…' sits in the cards it changes, not in a bar at the top", async () => {
  const page = await openPage();
  assert.equal(page.d.querySelectorAll("#verdict .segmented").length, 1);
  assert.equal(page.d.querySelectorAll("header .segmented, #tabs .segmented").length, 0);
  assert.match(page.text("#verdict .filters-hint")[0], /^Au repos : moins de 15 %/);
  click(page, [...page.d.querySelectorAll("#verdict .segment")].find((b) => b.textContent === "Charge légère"));
  await page.settle();
  assert.ok(page.requests.includes("/api/overview?load=light"));
  assert.match(page.text("#verdict")[0], /Depuis le début, light :/);
  page.close();
});

test("a card of the home page opens the curves with that component alone", async () => {
  const page = await openPage();
  const card = [...page.d.querySelectorAll(".comp-card")].find((c) => c.querySelector(".comp-title").textContent === "Boîtier");
  assert.match(card.getAttribute("href"), /page=curves/);
  assert.match(card.getAttribute("href"), /only=case/);
  page.close();
  const curves = await openPage({ page: "curves", hash: "&only=case" });
  assert.deepEqual([...curves.d.querySelectorAll(".picker input")].map((i) => i.checked), [false, false, false, true, false, false, false]);
  assert.match(labels(curves), /^Boîtier\|Au début : /, "one line, and the level of the beginning");
  curves.close();
});

test("too little history: the page prints the explanation it is given and invents no tiles", async () => {
  const overview = fixture("overview-idle.json");
  overview.headline = { state: "few", lead: "Pas encore assez d'historique pour comparer.", detail: "Pour dire si une température a changé, il faut au moins 60 jours de suivi.", tiles: [] };
  const page = await openPage({ overview });
  assert.match(page.text("#verdict")[0], /Pas encore assez d'historique pour comparer\..*au moins 60 jours/);
  assert.equal(page.d.querySelectorAll(".tile").length, 0);
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
  assert.equal(labels(page), "Processeur|Carte graphique|Carte mère|Boîtier|Disque 1|Disque 2|Disque 3");

  const swatches = [...page.d.querySelectorAll(".picker .swatch")].map((s) => s.style.background);
  assert.equal(new Set(swatches).size, 7, "one color per component");

  const disk1 = page.d.querySelectorAll(".picker input")[4];
  disk1.checked = false;
  change(page, disk1);
  assert.equal(labels(page), "Processeur|Carte graphique|Carte mère|Boîtier|Disque 2|Disque 3");
  assert.deepEqual([...page.d.querySelectorAll(".picker .swatch")].map((s) => s.style.background), swatches, "switching one off never repaints the others");

  click(page, [...page.d.querySelectorAll(".link-button")].find((b) => b.textContent === "Tout masquer"));
  assert.equal(page.d.querySelector("#curves .u-wrap-stub"), null);
  assert.match(page.text("#curves")[0], /Aucun composant affiché/);
  click(page, [...page.d.querySelectorAll(".link-button")].find((b) => b.textContent === "Tout afficher"));
  assert.ok(page.d.querySelector(".u-wrap-stub"));
  assert.deepEqual(page.errors, []);
  page.close();
});

test("the legend shows the figure the backend wrote, and a line that cannot be drawn is greyed with its reason", async () => {
  const temp = fixture("curves-idle-temp.json");
  temp.series[6].available = false;
  temp.series[6].note = "pas assez de recul";
  const page = await openPage({ page: "curves", curves: { temp } });
  assert.match(page.text(".series-now")[0], /^\d+,\d °C$/);
  assert.equal(page.text(".series-now")[6], "pas assez de recul");
  assert.ok(page.d.querySelectorAll(".picker input")[6].disabled);
  assert.ok(!labels(page).includes("Disque 3"));
  page.close();
});

test("the curves can show the change since the beginning, fetched from the backend", async () => {
  const page = await openPage({ page: "curves" });
  click(page, [...page.d.querySelectorAll("#curves .segment")].find((b) => b.textContent === "Écart depuis le début"));
  await page.settle();
  assert.ok(page.requests.includes("/api/curves?load=idle&mode=delta"));
  assert.match(page.text(".series-now")[0], /^[+−]?\d+,\d °C$/);
  assert.match(labels(page), /Au début$/, "a line marks the beginning (0)");
  assert.match(page.text("#curves .hint")[0], /Combien de degrés de plus ou de moins/);
  page.close();
});

test("the months table follows the lines that are shown", async () => {
  const page = await openPage({ page: "curves" });
  const heads = () => page.text("#curves .monthly thead th");
  assert.deepEqual(heads(), ["Mois", "Processeur", "Carte graphique", "Carte mère", "Boîtier", "Disque 1", "Disque 2", "Disque 3"]);
  const box = page.d.querySelectorAll(".picker input")[1];
  box.checked = false;
  change(page, box);
  assert.deepEqual(heads().slice(0, 3), ["Mois", "Processeur", "Carte mère"]);
  assert.match(page.text("#curves .monthly tbody tr")[0], /^avril 2026 ?48,4 °C ?41,0 °C/, "the cells of the shown lines only");
  page.close();
});

test("the filter 'Le PC est…' is right above the chart", async () => {
  const page = await openPage({ page: "curves" });
  const kids = [...page.d.querySelector("#curves").children].map((c) => c.className);
  assert.ok(kids.indexOf("load-filter") < kids.indexOf("curve-controls"));
  page.close();
});

// ── Details ──────────────────────────────────────────────────────────────────

test("the details list every sensor under its component, with the names the backend gives", async () => {
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
  assert.equal(names(0)[0], "Cœurs du processeur", "the sensor that stands for the component comes first");
  assert.ok(names(0).includes("Sonde de la carte mère, près du processeur"));
  assert.deepEqual(names(3), ["Intérieur du boîtier"]);
  assert.equal(names(6)[0], "Température du disque");
  assert.match(page.text("#components tbody tr")[0], /Cœurs du processeurCore \(Tctl\/Tdie\).*°C.*°C/);
  page.close();
});

test("a sensor opens in place with its story, its curve and its figures", async () => {
  const page = await openPage({ page: "details" });
  const row = [...page.d.querySelectorAll("#components tbody tr")].find((r) => r.textContent.startsWith("Température du disque"));
  click(page, row);
  await page.settle();
  assert.ok(page.requests.some((r) => r.startsWith("/api/sensor?") && r.includes("CT500P310SSD8")));
  assert.match(page.text(".detail .story")[0], /^Au début \(du .* au .*\), sa température habituelle était de \d+,\d °C\. Ces 30 derniers jours : \d+,\d °C, soit .*de plus qu'au début\.$/);
  assert.equal(page.d.querySelectorAll(".detail .story strong").length, 2, "the figures are in bold");
  // The advice weighs the sensor against the rest of the machine before naming a cause.
  assert.match(page.text(".detail .advice")[0], /reste de la machine/);
  assert.match(page.text(".detail .muted")[0], /^Disque 1 · CT500P310SSD8 · au repos/);
  assert.equal(labels(page).split("|").length, 2, "one line plus the beginning");
  assert.match(page.text(".detail details")[1], /Pente générale/);
  assert.ok(page.d.querySelector('#components tr[aria-expanded="true"]'));
  click(page, page.d.querySelector('#components tr[aria-expanded="true"]'));
  assert.equal(page.d.querySelector(".detail"), null, "a second click closes it");
  assert.deepEqual(page.errors, []);
  page.close();
});

test("the same month a year apart is shown as its own table, open", async () => {
  const sensor = fixture("sensor-disk1.json");
  sensor.seasons = [
    { label: "novembre 2026", before: "48,8 °C", after: "52,9 °C", change: "+4,1 °C" },
    { label: "octobre 2026", before: "50,1 °C", after: "53,7 °C", change: "+3,6 °C" },
  ];
  const page = await openPage({ page: "details", sensor });
  const row = [...page.d.querySelectorAll("#components tbody tr")].find((r) => r.textContent.startsWith("Température du disque"));
  click(page, row);
  await page.settle();
  const seasons = [...page.d.querySelectorAll(".detail details")].find((d) => d.querySelector("summary").textContent === "Le même mois, un an avant");
  assert.ok(seasons, "the season comparison has its own section");
  assert.ok(seasons.hasAttribute("open"), "it is the comparison the season cannot distort: open by default");
  assert.deepEqual([...seasons.querySelectorAll("thead th")].map((c) => c.textContent), ["Mois", "Un an avant", "Ce mois-ci", "Écart"]);
  const rows = [...seasons.querySelectorAll("tr")].filter((tr) => tr.querySelector('th[scope="row"]'));
  assert.deepEqual(
    rows.map((tr) => [...tr.children].map((c) => c.textContent)),
    [
      ["novembre 2026", "48,8 °C", "52,9 °C", "+4,1 °C"],
      ["octobre 2026", "50,1 °C", "53,7 °C", "+3,6 °C"],
    ],
  );
  page.close();
});

test("without thirteen months of history no season table is drawn", async () => {
  const page = await openPage({ page: "details" });
  const row = [...page.d.querySelectorAll("#components tbody tr")].find((r) => r.textContent.startsWith("Température du disque"));
  click(page, row);
  await page.settle();
  const summaries = [...page.d.querySelectorAll(".detail summary")].map((s) => s.textContent);
  assert.ok(!summaries.includes("Le même mois, un an avant"), summaries.join("|"));
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



test("the status page says where the data is and for how long it is kept, as the backend wrote it", async () => {
  const page = await openPage({ page: "status" });
  const [daily, raw, file] = page.text("#data li");
  assert.match(daily, /Résumé de chaque jour pour tous les capteurs, depuis le 8 avril 2026 : conservé sans limite/);
  assert.match(raw, /Mesures détaillées \(une toutes les 5 minutes\) : conservées 365 jours/);
  assert.match(file, /Fichier : E:\\monitoring\.db\\temperatures\.db \(1,8 Mo\)/);
  page.close();
});
