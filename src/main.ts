import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

let selectDirBtn: HTMLButtonElement | null;
let mergeBtn: HTMLButtonElement | null;
let selectedDirEl: HTMLElement | null;
let mergeMsgEl: HTMLElement | null;
let selectedDir: string | null = null;

async function selectDirectory() {
  const dir = await open({
    directory: true,
    multiple: false,
  });
  if (dir && typeof dir === "string") {
    selectedDir = dir;
    if (selectedDirEl) {
      selectedDirEl.textContent = `Selected: ${dir}`;
    }
    if (mergeBtn) {
      mergeBtn.disabled = false;
    }
  }
}

async function mergePdfs() {
  if (!selectedDir) return;
  if (mergeMsgEl) {
    mergeMsgEl.textContent = "Merging...";
  }
  try {
    const result = await invoke("merge_pdfs", { dirPath: selectedDir });
    if (mergeMsgEl) {
      mergeMsgEl.textContent = result as string;
    }
  } catch (error) {
    if (mergeMsgEl) {
      mergeMsgEl.textContent = `Error: ${error}`;
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  selectDirBtn = document.querySelector("#select-dir-btn");
  mergeBtn = document.querySelector("#merge-btn");
  selectedDirEl = document.querySelector("#selected-dir");
  mergeMsgEl = document.querySelector("#merge-msg");

  selectDirBtn?.addEventListener("click", selectDirectory);
  mergeBtn?.addEventListener("click", mergePdfs);
});
