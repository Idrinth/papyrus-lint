import { invoke } from "@tauri-apps/api/core";

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;

async function greet() {
  if (greetMsgEl && greetInputEl) {
    try {
      // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
      greetMsgEl.textContent = await invoke("greet", {
        name: greetInputEl.value,
      });
    } catch (error) {
      greetMsgEl.textContent = "Something went wrong. Please try again.";
      console.error(error);
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document.querySelector("#greet-form")?.addEventListener("submit", (e) => {
    e.preventDefault();
    greet();
  });
});
