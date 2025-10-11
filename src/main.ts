import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Command } from "@tauri-apps/plugin-shell";

let selectDirBtn: HTMLButtonElement | null;
let mergeBtn: HTMLButtonElement | null;
let selectedDirEl: HTMLElement | null;
let mergeMsgEl: HTMLElement | null;
let openFolderBtn: HTMLButtonElement | null;
let selectedDir: string | null = null;
let mergedFilePath: string | null = null;

async function selectDirectory() {
  const dir = await open({
    directory: true,
    multiple: false,
  });
  if (dir && typeof dir === "string") {
    selectedDir = dir;
    
    // Count PDFs in the selected directory
    try {
      const count = await invoke("count_pdfs", { dirPath: dir });
      if (selectedDirEl) {
        selectedDirEl.textContent = `선택됨: ${dir}\nPDF 파일 개수: ${count}개`;
      }
      if (mergeBtn) {
        mergeBtn.disabled = count === 0;
      }
    } catch (error) {
      if (selectedDirEl) {
        selectedDirEl.textContent = `선택됨: ${dir}\n오류: ${error}`;
      }
      if (mergeBtn) {
        mergeBtn.disabled = true;
      }
    }
  }
}

async function mergePdfs() {
  if (!selectedDir) return;
  if (mergeMsgEl) {
    mergeMsgEl.textContent = "병합 중...";
  }
  try {
    const filePath = await invoke("merge_pdfs", { dirPath: selectedDir });
    mergedFilePath = filePath as string;
    if (mergeMsgEl) {
      mergeMsgEl.textContent = `✅ 병합 완료: ${mergedFilePath}`;
    }
    if (openFolderBtn) {
      openFolderBtn.disabled = false;
    }
  } catch (error) {
    if (mergeMsgEl) {
      mergeMsgEl.textContent = `❌ 오류: ${error}`;
    }
  }
}

async function openFolder() {
  if (!mergedFilePath) return;
  
  try {
    // Get the directory containing the merged file
    const dirPath = mergedFilePath.substring(0, mergedFilePath.lastIndexOf("\\"));
    
    // Open the folder in Windows Explorer
    const command = Command.create("explorer", [dirPath]);
    await command.execute();
  } catch (error) {
    if (mergeMsgEl) {
      mergeMsgEl.textContent = `❌ 폴더 열기 오류: ${error}`;
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  selectDirBtn = document.querySelector("#select-dir-btn");
  mergeBtn = document.querySelector("#merge-btn");
  selectedDirEl = document.querySelector("#selected-dir");
  mergeMsgEl = document.querySelector("#merge-msg");
  openFolderBtn = document.querySelector("#open-folder-btn");

  selectDirBtn?.addEventListener("click", selectDirectory);
  mergeBtn?.addEventListener("click", mergePdfs);
  openFolderBtn?.addEventListener("click", openFolder);
});
