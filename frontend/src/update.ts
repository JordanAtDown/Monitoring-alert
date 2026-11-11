import type { UpdateInfo } from "./api";
import { clear, h } from "./dom";
import { fmtDay } from "./format";

/** Only GitHub links are ever followed (the server filters them too). */
function safeUrl(url: string | null | undefined): string | null {
  return url && url.startsWith("https://github.com/") ? url : null;
}

/** The button of the header: only there when a newer version exists. */
export function renderUpdateButton(slot: HTMLElement, info: UpdateInfo | null, dialog: HTMLDialogElement): void {
  clear(slot);
  if (!info || !info.available || !info.latest) return;
  const button = h(
    "button",
    { type: "button", class: "update-button", "aria-haspopup": "dialog" },
    h("span", { "aria-hidden": "true" }, "⬆ "),
    `Mise à jour disponible : version ${info.latest.version}`,
  );
  button.addEventListener("click", () => {
    if (!dialog.open) dialog.showModal();
  });
  slot.append(button);
}

/** The window that explains how to update. */
export function fillUpdateDialog(dialog: HTMLDialogElement, info: UpdateInfo | null): void {
  clear(dialog);
  const latest = info?.latest;
  if (!info || !latest || !info.available) {
    dialog.append(h("p", {}, "Vous avez la dernière version."), closeButton(dialog));
    return;
  }
  const download = safeUrl(latest.download_url);
  const page = safeUrl(latest.page_url);
  const file = `monitoring_${latest.version}.exe`;

  const steps = h("ol", { class: "steps" });
  steps.append(
    h(
      "li",
      {},
      h("strong", {}, "Téléchargez le programme d'installation."),
      h("div", { class: "step-action" }, download
        ? h("a", { class: "primary-link", href: download, rel: "noopener noreferrer" }, `Télécharger ${file}`)
        : page
          ? h("a", { class: "primary-link", href: page, target: "_blank", rel: "noopener noreferrer" }, "Ouvrir la page de la version")
          : null),
    ),
    h(
      "li",
      {},
      h("strong", {}, "Lancez le fichier téléchargé."),
      h("p", {}, `Il s'appelle ${file} et se trouve dans votre dossier Téléchargements. Si Windows affiche « Windows a protégé votre ordinateur », cliquez sur « Informations complémentaires », puis sur « Exécuter quand même ».`),
    ),
    h(
      "li",
      {},
      h("strong", {}, "Suivez l'assistant (Suivant, Installer)."),
      h("p", {}, "Vos mesures et votre configuration sont conservées. Le programme redémarre tout seul : revenez ensuite sur cette page."),
    ),
  );

  dialog.append(
    h("h2", {}, "Une nouvelle version est disponible"),
    h("p", { class: "versions" }, `Vous avez la version ${info.current}. La version ${latest.version} est disponible${latest.published_at ? `, publiée le ${fmtDay(latest.published_at.slice(0, 10))}` : ""}.`),
    h("h3", {}, "Comment mettre à jour"),
    steps,
  );
  if (latest.notes) {
    dialog.append(h("details", { class: "notes" }, h("summary", {}, "Ce qui change dans cette version"), h("div", { class: "notes-text" }, latest.notes)));
  }
  if (page) {
    dialog.append(h("p", {}, h("a", { href: page, target: "_blank", rel: "noopener noreferrer" }, "Voir la page de la version sur GitHub")));
  }
  dialog.append(closeButton(dialog));
}

function closeButton(dialog: HTMLDialogElement): HTMLElement {
  const button = h("button", { type: "button", class: "close-button" }, "Fermer");
  button.addEventListener("click", () => dialog.close());
  return h("div", { class: "dialog-actions" }, button);
}

/** One line for the footer: which version runs, and whether it is the latest. */
export function versionLine(version: string | null, info: UpdateInfo | null): string {
  const v = version ?? info?.current ?? "?";
  if (!info) return `Monitoring ${v}`;
  if (!info.enabled) return `Monitoring ${v} — recherche de mises à jour désactivée`;
  if (info.available && info.latest) return `Monitoring ${v} — la version ${info.latest.version} est disponible`;
  if (info.latest) return `Monitoring ${v} — à jour`;
  if (info.error) return `Monitoring ${v} — impossible de chercher les mises à jour pour le moment`;
  return `Monitoring ${v} — recherche de mises à jour en cours…`;
}
