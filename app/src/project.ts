// Project directory, compiler path, and script-root settings. Implementation
// lives in the sibling project-*.ts modules; this file is the production
// façade so callers don't have to know which slice they need.

export type { ProjectInfo } from "./project-state";
export {
  configPathOverride,
  currentCompileCheck,
  currentCompilerPath,
  currentLookupScriptRoots,
  currentProjectDir,
  effectiveScriptRoots,
  setAchlistScriptRoots,
  setPpjImportRoots,
} from "./project-state";
export {
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPpj,
  projectDirForPscPath,
} from "./project-io";
export { bindProjectSettings, loadProjectConfig, useProjectDir } from "./project-settings";
