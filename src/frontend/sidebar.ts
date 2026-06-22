import { NAV_ITEMS } from "./state";
import type { PanelId } from "./types";

export function renderSidebar(currentPanel: PanelId): string {
  return NAV_ITEMS.map((it) => `
    <li class="nav-item ${it.id === currentPanel ? "active" : ""}" data-panel="${it.id}">
      <span class="nav-label">${it.label}</span>
    </li>
  `).join("");
}

export function attachSidebarListeners(onNavigate: (panel: PanelId) => void): void {
  for (const li of Array.from(document.querySelectorAll<HTMLElement>(".nav-item"))) {
    li.addEventListener("click", () => {
      onNavigate(li.dataset.panel as PanelId);
    });
  }
}