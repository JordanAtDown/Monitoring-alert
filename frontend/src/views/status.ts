// "État": is everything working? Plus where the data is and which version runs.

import { clear, h } from "../dom";
import { fmtBytes, fmtDay, fmtInt } from "../format";
import { LEVEL_ICON, state } from "../state";

/** The tab title shows the state too: a pinned tab is enough to see that all is well. */
export function setTabTitle(): void {
  const health = state.health;
  const problems = health ? health.checks.filter((c) => c.level === "warn" || c.level === "bad").length : 0;
  document.title = !health ? "✕ Monitoring" : health.overall === "ok" ? "✓ Monitoring" : `${LEVEL_ICON[health.overall]} Monitoring (${problems})`;
}

/** The small badge of the header, on every page: a link to the "État" page. */
export function renderPill(host: HTMLElement, href: string): void {
  clear(host);
  const health = state.health;
  const kind = !health ? "bad" : health.overall === "ok" ? "ok" : health.overall === "warn" ? "warn" : "bad";
  host.append(
    h(
      "a",
      { class: `pill pill-${kind}`, href },
      h("span", { class: "status-icon", "aria-hidden": "true" }, health ? LEVEL_ICON[health.overall] : "✕"),
      health ? health.summary : "Programme injoignable",
    ),
  );
}

function banner(): HTMLElement[] {
  const health = state.health;
  if (!health) {
    return [
      h("p", { class: "status status-bad" }, h("span", { class: "status-icon", "aria-hidden": "true" }, "✕"), "La page ne parvient plus à joindre le programme."),
      h("p", { class: "status-detail" }, "Il est peut-être arrêté : lancez Monitoring depuis le menu Démarrer."),
    ];
  }
  const kind = health.overall === "ok" ? "ok" : health.overall === "warn" ? "warn" : "bad";
  const out: HTMLElement[] = [h("p", { class: `status status-${kind}` }, h("span", { class: "status-icon", "aria-hidden": "true" }, LEVEL_ICON[health.overall]), health.summary)];
  const problems = health.checks.filter((c) => c.level === "warn" || c.level === "bad");
  if (problems.length > 0) {
    const list = h("ul", { class: "problems" });
    for (const c of problems) list.append(h("li", {}, h("strong", {}, `${c.title} : `), c.message));
    out.push(list);
  }
  return out;
}

function checksCard(): HTMLElement {
  const card = h("section", { id: "health", class: "card", "aria-live": "polite" }, h("h2", {}, "Tout fonctionne-t-il ?"), ...banner());
  if (state.health) {
    const items = h("ul", { class: "check-list" });
    for (const c of state.health.checks) {
      items.append(
        h("li", { class: `check check-${c.level}` }, h("span", { class: "check-icon", "aria-hidden": "true" }, LEVEL_ICON[c.level]), h("span", { class: "check-title" }, c.title), h("span", { class: "check-message" }, c.message)),
      );
    }
    card.append(items);
  }
  return card;
}

function dataCard(): HTMLElement | null {
  const s = state.status;
  if (!s) return null;
  const minutes = Math.round(s.collector.interval_secs / 60);
  return h(
    "section",
    { id: "data", class: "card" },
    h("h2", {}, "Vos données"),
    h(
      "ul",
      { class: "data-list" },
      h("li", {}, h("strong", {}, "Résumé de chaque jour"), ` pour tous les capteurs${s.db.history_since ? `, depuis le ${fmtDay(s.db.history_since)}` : ""} : conservé sans limite. C'est lui qui permet de suivre l'évolution sur des mois.`),
      h("li", {}, h("strong", {}, "Mesures détaillées"), ` (${minutes === 1 ? "une par minute" : `une toutes les ${fmtInt(minutes)} minutes`}) : conservées ${fmtInt(s.db.raw_retention_days)} jours, ${fmtInt(s.db.total_snapshots)} relevés actuellement. Avant d'être supprimées, elles sont résumées.`),
      h("li", {}, h("strong", {}, "Fichier"), ` : ${s.db.path} (${fmtBytes(s.db.size_bytes)})`),
    ),
  );
}

export function renderStatus(host: HTMLElement): void {
  clear(host);
  host.append(checksCard());
  const data = dataCard();
  if (data) host.append(data);
}
