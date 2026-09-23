import { type ConfigSelectionResult, isSelectableGame } from "./main-types";
import { type ProjectInfo } from "./backend-types";
import { loadConfigPresets } from "./presets-api";
import { configPickerContinueEl, configPickerDetectedEl, configPickerDetectedPathEl, configPickerEl, configPickerGameEl, configPickerGameSelectEl, configPickerNoneEl, configPickerPathInputEl, configPickerPresetListEl, configPickerUsePathButtonEl } from "./presets-state";
// Shows the "select this project's configuration" dialog useProjectDir
// opens for every not-yet-confirmed project directory (see
// confirmedProjectDirs), so a project's configuration is always picked
// with the project itself already known, rather than the Settings tab
// showing/editing whatever configuration happened to be loaded previously
// (or the engine's silent defaults) before the user has even said which
// project it applies to. Resolves to `{ kind: "detected" }` for "Continue"
// (or Escape/a backdrop click), which leaves useProjectDir's own
// auto-detection to do the right thing whether or not the project already
// has a configuration file; to `{ kind: "path", path }` once a non-blank
// path is confirmed via the "different file" input; or to
// `{ kind: "preset", preset }` once one of the inline preset options -
// shown only when the project has no configuration file yet, since
// initializing from a preset requires there to be none (see
// applyConfigPreset/papyrus_lint_config::initialize_default_config)
// - is clicked. A project with no configuration file also gets `game`
// on Continue and on a preset choice, from the dialog's target-game
// select, so loadProjectConfig can stamp that key into the new file.
// Resolves immediately with `{ kind: "detected" }` if the
// dialog isn't present in the DOM (e.g. a minimal test fixture).
export async function promptForConfigSelection(projectInfo: ProjectInfo): Promise<ConfigSelectionResult> {
  if (!configPickerEl) {
    return { kind: "detected" };
  }

  const detectedPath = projectInfo.used_configuration_file;
  const presets = detectedPath ? [] : await loadConfigPresets();
  const dialog = configPickerEl;

  return new Promise((resolve) => {
    if (configPickerDetectedEl) {
      configPickerDetectedEl.hidden = !detectedPath;
    }
    if (configPickerDetectedPathEl) {
      configPickerDetectedPathEl.textContent = detectedPath ?? "";
    }
    if (configPickerNoneEl) {
      configPickerNoneEl.hidden = Boolean(detectedPath);
    }
    if (configPickerGameEl) {
      configPickerGameEl.hidden = Boolean(detectedPath);
    }
    if (!detectedPath && configPickerGameSelectEl) {
      configPickerGameSelectEl.value = "skyrim";
    }
    if (configPickerPathInputEl) {
      configPickerPathInputEl.value = "";
    }

    let settled = false;
    // configPickerContinueEl/configPickerUsePathButtonEl are static
    // elements reused across every call (unlike the preset options below,
    // rebuilt fresh each time), so their listeners must be explicitly torn
    // down here - otherwise an earlier, already-resolved call's handler
    // (still bound, since a run that resolved via a different path/Escape
    // never fired it to trigger its own removal) would keep piling up
    // across every project dropped in the session.
    const cleanup = () => {
      dialog.removeEventListener("close", handleClose);
      configPickerContinueEl?.removeEventListener("click", handleContinue);
      configPickerUsePathButtonEl?.removeEventListener("click", handleUsePath);
    };
    const finish = (result: ConfigSelectionResult) => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      if (dialog.hasAttribute("open")) {
        dialog.close();
      }
      resolve(result);
    };
    const handleClose = () => finish({ kind: "detected" });
    const handleContinue = () => {
      finish(detectedPath ? { kind: "detected" } : { kind: "detected", game: pickerGame() });
    };
    const handleUsePath = () => {
      const path = configPickerPathInputEl?.value.trim();
      if (path) {
        finish({ kind: "path", path });
      }
    };

    configPickerContinueEl?.addEventListener("click", handleContinue);
    configPickerUsePathButtonEl?.addEventListener("click", handleUsePath);

    if (configPickerPresetListEl) {
      configPickerPresetListEl.hidden = presets.length === 0;
      configPickerPresetListEl.innerHTML = "";
      for (const preset of presets) {
        const option = document.createElement("button");
        option.type = "button";
        option.className = "config-picker__preset-option";
        const label = document.createElement("strong");
        label.className = "config-picker__preset-option-label";
        label.textContent = preset.label;
        const description = document.createElement("span");
        description.className = "config-picker__preset-option-description";
        description.textContent = preset.description;
        option.append(label, description);
        option.addEventListener("click", () => {
          finish({ kind: "preset", preset: preset.id, game: pickerGame() });
        });
        configPickerPresetListEl.appendChild(option);
      }
    }

    dialog.addEventListener("close", handleClose, { once: true });
    dialog.showModal();
  });
}

function pickerGame() {
  const value = configPickerGameSelectEl?.value;
  return isSelectableGame(value) ? value : "skyrim";
}
