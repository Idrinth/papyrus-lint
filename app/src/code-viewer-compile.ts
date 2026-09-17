import { compilePscFile } from "./backend";

// Shows `text` in `outputEl`, styling it as a success or failure so a
// failed compile is easy to spot at a glance.
function showCompileOutput(outputEl: HTMLElement, text: string, success: boolean) {
  outputEl.textContent = text;
  outputEl.hidden = false;
  outputEl.classList.toggle("psc-result__compile-output--ok", success);
  outputEl.classList.toggle("psc-result__compile-output--error", !success);
}

// Hides and clears a previous compile result, if any is showing (e.g. from
// an earlier file in the same code viewer session).
export function hideCompileOutput(outputEl: HTMLElement | null) {
  if (!outputEl) {
    return;
  }
  outputEl.hidden = true;
  outputEl.textContent = "";
  outputEl.classList.remove("psc-result__compile-output--ok", "psc-result__compile-output--error");
}

// Compiles `path` via PapyrusCompiler.exe and shows the result in
// `outputEl`, reporting both a successful compile and a compiler-reported
// failure (syntax errors, missing imports, etc.) as well as a failure to
// run the compiler at all (e.g. no path configured). Shared by the "Compile"
// button on the Lint results list and the code viewer's "Save & Compile"
// button.
export async function compileAndShowOutput(path: string, outputEl: HTMLElement): Promise<void> {
  try {
    const outcome = await compilePscFile(path);
    const lines = [outcome.stdout, outcome.stderr].filter((text) => text.trim().length > 0);
    if (outcome.personal_data_stripped) {
      lines.push("Removed your username/computer name from the compiled script.");
    }
    const output = lines.join("\n");
    showCompileOutput(
      outputEl,
      output || (outcome.success ? "Compiled successfully." : "Compilation failed."),
      outcome.success,
    );
  } catch (error) {
    showCompileOutput(outputEl, String(error), false);
    console.error(error);
  }
}
