import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";

type ProjectOption = { id: number; name: string; branch: string | null };
type ModelOption = {
  modelSlug: string;
  displayName: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
};
type Preference = {
  roleId: string;
  roleKind: "main_overseer" | "subagent_default" | "named_agent";
  agentName: string | null;
  modelSlug: string;
  reasoningEffort: string;
  catalogVersion: string;
  lastAppliedTarget: string | null;
  lastAppliedAt: number | null;
  lastAppliedConfigHash: string | null;
  updatedAt: number;
};
type SelectorSnapshot = {
  catalog: {
    source: string;
    cliVersion: string;
    accountAvailabilityVerified: boolean;
    models: ModelOption[];
  };
  preferences: Preference[];
  preferenceState: string;
  runtimeEffectivenessVerified: boolean;
};
type PreferenceInput = {
  roleKind: Preference["roleKind"];
  agentName: string | null;
  modelSlug: string;
  reasoningEffort: string;
};
type ConfigTarget =
  | { kind: "project"; projectId: number }
  | { kind: "user_base" }
  | { kind: "user_profile"; profileName: string }
  | { kind: "export_only" };
type ConfigPreview = {
  previewId: string;
  target: string;
  changes: { file: string; key: string; currentValue: string | null; selectedValue: string }[];
  skippedRoles: { roleId: string; reason: string }[];
  exportText: string;
  canApply: boolean;
  exportOnly: boolean;
};
type ApplyResult = {
  target: string;
  preferenceState: string;
  backupCount: number;
  configHash: string;
  runtimeEffectivenessVerified: boolean;
};
type Draft = { modelSlug: string; reasoningEffort: string };

function makeDraft(catalog: SelectorSnapshot["catalog"] | null, preference?: Preference): Draft {
  const model = catalog?.models.find((option) => option.modelSlug === preference?.modelSlug)
    ?? catalog?.models[0];
  return {
    modelSlug: preference?.modelSlug ?? model?.modelSlug ?? "",
    reasoningEffort: preference?.reasoningEffort ?? model?.defaultReasoningEffort ?? model?.reasoningEfforts[0] ?? "",
  };
}

export default function CodexSettings({
  projects,
  onBack,
}: {
  projects: ProjectOption[];
  onBack: () => void;
}) {
  const [snapshot, setSnapshot] = useState<SelectorSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [mainDraft, setMainDraft] = useState<Draft>({ modelSlug: "", reasoningEffort: "" });
  const [subagentDraft, setSubagentDraft] = useState<Draft>({ modelSlug: "", reasoningEffort: "" });
  const [customName, setCustomName] = useState("");
  const [customDraft, setCustomDraft] = useState<Draft>({ modelSlug: "", reasoningEffort: "" });
  const [targetKind, setTargetKind] = useState<ConfigTarget["kind"]>("user_base");
  const [projectId, setProjectId] = useState(projects[0]?.id ?? 0);
  const [profileName, setProfileName] = useState("default");
  const [preview, setPreview] = useState<ConfigPreview | null>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);

  const refresh = async () => {
    const next = await invoke<SelectorSnapshot>("codex_selector_snapshot");
    setSnapshot(next);
    setMainDraft(makeDraft(next.catalog, next.preferences.find((item) => item.roleKind === "main_overseer")));
    setSubagentDraft(makeDraft(next.catalog, next.preferences.find((item) => item.roleKind === "subagent_default")));
    const named = next.preferences.find((item) => item.roleKind === "named_agent" && item.agentName === customName.trim())
      ?? next.preferences.find((item) => item.roleKind === "named_agent");
    if (named?.agentName) {
      setCustomName(named.agentName);
      setCustomDraft(makeDraft(next.catalog, named));
    } else {
      setCustomDraft(makeDraft(next.catalog));
    }
  };

  useEffect(() => {
    titleRef.current?.focus();
    refresh().catch((reason: unknown) => {
      setError(reason instanceof Error ? reason.message : String(reason));
    });
  }, []);

  const target = useMemo<ConfigTarget>(() => {
    if (targetKind === "project") return { kind: "project", projectId };
    if (targetKind === "user_profile") return { kind: "user_profile", profileName };
    if (targetKind === "export_only") return { kind: "export_only" };
    return { kind: "user_base" };
  }, [targetKind, projectId, profileName]);

  const updateDraft = (
    setter: (next: Draft | ((current: Draft) => Draft)) => void,
    current: Draft,
    field: keyof Draft,
    value: string,
  ) => {
    if (field === "modelSlug") {
      const nextModel = snapshot?.catalog.models.find((model) => model.modelSlug === value);
      setter({
        modelSlug: value,
        reasoningEffort: nextModel?.reasoningEfforts.includes(current.reasoningEffort)
          ? current.reasoningEffort
          : nextModel?.defaultReasoningEffort ?? nextModel?.reasoningEfforts[0] ?? "",
      });
    } else {
      setter({ ...current, reasoningEffort: value });
    }
  };

  const savePreference = async (input: PreferenceInput, label: string) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await discardPreview();
      const saved = await invoke<Preference>("save_codex_selector_preference", { input });
      await refresh();
      setNotice(`${label} saved in Cathulu (${saved.modelSlug} · ${saved.reasoningEffort}). It has not changed the Codex config yet.`);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };

  const discardPreview = async () => {
    const current = preview;
    setPreview(null);
    if (current) {
      try {
        await invoke("discard_codex_config_preview", { previewId: current.previewId });
      } catch {
        // The preview is local and disposable; clearing the view is still safe.
      }
    }
  };

  const preparePreview = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      if (preview) await invoke("discard_codex_config_preview", { previewId: preview.previewId });
      setPreview(null);
      const next = await invoke<ConfigPreview>("preview_codex_config", { target });
      setPreview(next);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setError(message);
      if (message.includes("changed after preview") || message.includes("preview expired")) setPreview(null);
    } finally {
      setBusy(false);
    }
  };

  const applyPreview = async () => {
    if (!preview?.canApply) return;
    setBusy(true);
    setError(null);
    try {
      const result = await invoke<ApplyResult>("apply_codex_config_preview", {
        previewId: preview.previewId,
      });
      setPreview(null);
      await refresh();
      setNotice(`Written to ${result.target}. Cathulu created ${result.backupCount} backup${result.backupCount === 1 ? "" : "s"}; runtime effectiveness remains unverified.`);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setPreview(null);
    } finally {
      setBusy(false);
    }
  };

  const currentPreference = (roleKind: Preference["roleKind"], name?: string) =>
    snapshot?.preferences.find((item) => item.roleKind === roleKind && (roleKind !== "named_agent" || item.agentName === name));

  return (
    <main className="main-content settings-content">
      <div className="settings-topline">
        <button className="text-button" type="button" onClick={onBack}>Back to dashboard</button>
        <span className="local-indicator"><span /> Local settings</span>
      </div>
      <section className="settings-intro" aria-labelledby="settings-title">
        <div>
          <h1 ref={titleRef} id="settings-title" tabIndex={-1}>Codex settings</h1>
          <p>Choose model and effort preferences for future Codex sessions. Cathulu stores choices locally, then shows the exact configuration changes before you apply them.</p>
        </div>
        <div className="settings-state">
          <span>Current state</span>
          <strong>{snapshot?.preferenceState === "written_to_config" ? "Written to config" : "Stored in Cathulu"}</strong>
          <small>Runtime effectiveness unverified</small>
        </div>
      </section>

      {error && <div className="settings-message settings-error" role="alert">{error}</div>}
      {notice && <div className="settings-message settings-notice" role="status">{notice}</div>}
      {!snapshot ? (
        <div className="state-panel" role="status"><span className="loading-mark" /><p>Reading the local Codex model catalog…</p></div>
      ) : (
        <>
          <p className="catalog-note">
            Options come from {snapshot.catalog.source.replace(/_/g, " ")} ({snapshot.catalog.cliVersion}). The local catalog does not verify account or workspace availability.
          </p>
            {snapshot.preferences.length === 0 && (
              <p className="catalog-note">Save at least one preference before Cathulu can prepare a configuration preview.</p>
            )}

          <div className="selector-list">
            <PreferenceRow
              title="Main Overseer"
              description="Sets the model and reasoning effort for the main Codex session in the selected configuration file."
              modelOptions={snapshot.catalog.models}
              draft={mainDraft}
              saved={currentPreference("main_overseer")}
              busy={busy}
              onChange={(field, value) => updateDraft(setMainDraft, mainDraft, field, value)}
              onSave={() => savePreference({ roleKind: "main_overseer", agentName: null, ...mainDraft }, "Main Overseer preference")}
            />
            <PreferenceRow
              title="Default sub-agents"
              description="Sets the default for sub-agents. Explicit model or effort overrides used when spawning an agent remain higher priority."
              modelOptions={snapshot.catalog.models}
              draft={subagentDraft}
              saved={currentPreference("subagent_default")}
              busy={busy}
              onChange={(field, value) => updateDraft(setSubagentDraft, subagentDraft, field, value)}
              onSave={() => savePreference({ roleKind: "subagent_default", agentName: null, ...subagentDraft }, "Default sub-agent preference")}
            />
            <section className="selector-row" aria-labelledby="named-agent-title">
              <div className="selector-heading">
                <h2 id="named-agent-title">Named agent</h2>
                <p>Overrides one existing custom agent file. Cathulu will not create a partial agent definition.</p>
              </div>
              <div className="selector-fields named-agent-fields">
                <label>
                  <span>Agent name</span>
                  <input value={customName} maxLength={80} onChange={(event) => setCustomName(event.target.value)} placeholder="e.g. reviewer" />
                </label>
                <ModelFields
                  label="Named agent"
                  modelOptions={snapshot.catalog.models}
                  draft={customDraft}
                  onChange={(field, value) => updateDraft(setCustomDraft, customDraft, field, value)}
                />
                <button
                  className="primary-button"
                  type="button"
                  disabled={busy || !customName.trim() || !customDraft.modelSlug || !customDraft.reasoningEffort}
                  onClick={() => savePreference({ roleKind: "named_agent", agentName: customName.trim(), ...customDraft }, `Agent ${customName.trim()} preference`)}
                >
                  Save preference
                </button>
              </div>
              {currentPreference("named_agent", customName.trim()) && (
                <SavedState preference={currentPreference("named_agent", customName.trim())!} />
              )}
            </section>
          </div>

          <section className="config-target" aria-labelledby="target-title">
            <div className="config-target-heading">
              <div>
                <h2 id="target-title">Configuration target</h2>
                <p>Choose where the saved preferences should be written, or export a copyable snippet.</p>
              </div>
            </div>
            <div className="target-fields">
              <label>
                <span>Save to</span>
                <select
                  value={targetKind}
                  onChange={(event) => {
                    void discardPreview();
                    setTargetKind(event.target.value as ConfigTarget["kind"]);
                  }}
                >
                  <option value="user_base">User config</option>
                  <option value="user_profile">User profile config</option>
                  {projects.length > 0 && <option value="project">Project config</option>}
                  <option value="export_only">Export only</option>
                </select>
              </label>
              {targetKind === "project" && (
                <label>
                  <span>Registered project</span>
                  <select value={projectId} onChange={(event) => { void discardPreview(); setProjectId(Number(event.target.value)); }}>
                    {projects.map((project) => <option key={project.id} value={project.id}>{project.name}{project.branch ? ` · ${project.branch}` : ""}</option>)}
                  </select>
                </label>
              )}
              {targetKind === "user_profile" && (
                <label>
                  <span>Profile name</span>
                  <input value={profileName} maxLength={80} onChange={(event) => { void discardPreview(); setProfileName(event.target.value); }} />
                </label>
              )}
              <button className="secondary-button" type="button" disabled={busy || !snapshot.preferences.length} onClick={preparePreview}>
                Review configuration changes
              </button>
            </div>
            {targetKind === "project" && (
              <p className="target-note">Codex reads project settings only when the repository is trusted. Changes apply to a new session after Codex loads this project configuration.</p>
            )}
            {targetKind !== "project" && targetKind !== "export_only" && (
              <p className="target-note">User and profile settings affect new sessions when that config or profile is selected. Cathulu cannot change a session already running.</p>
            )}
          </section>

          {preview && (
            <section className="config-preview" aria-labelledby="preview-title">
              <div className="preview-heading">
                <div>
                  <h2 id="preview-title">Review before applying</h2>
                  <p>{preview.target}</p>
                </div>
                <button className="text-button" type="button" disabled={busy} onClick={() => void discardPreview()}>Discard preview</button>
              </div>
              {preview.changes.length === 0 ? (
                <p className="panel-empty">No file changes are needed for the selected preferences.</p>
              ) : (
                <div className="change-list">
                  {preview.changes.map((change) => (
                    <div className="change-row" key={`${change.file}:${change.key}`}>
                      <div><code>{change.file}</code><strong>{change.key}</strong></div>
                      <span>{change.currentValue ?? "not set"}</span>
                      <span aria-hidden="true">→</span>
                      <strong>{change.selectedValue}</strong>
                    </div>
                  ))}
                </div>
              )}
              {preview.skippedRoles.length > 0 && (
                <ul className="skipped-role-list" aria-label="Preferences not included in this target">
                  {preview.skippedRoles.map((role) => (
                    <li key={role.roleId}><strong>{role.roleId}</strong><span>{role.reason}</span></li>
                  ))}
                </ul>
              )}
              <details className="export-details">
                <summary>Show copyable TOML snippet</summary>
                <pre>{preview.exportText}</pre>
              </details>
              <p className="preview-safety">
                {preview.exportOnly
                  ? "This copyable snippet contains only the selected keys. Cathulu will not read config contents or change any configuration file. "
                  : preview.canApply
                    ? "Only the listed model and effort keys will change. Existing comments and unrelated settings are preserved. Cathulu creates a backup before each write. "
                    : "No configuration file changes are needed for the preferences that match this target. "}
                Runtime effectiveness remains unverified until Codex reports the selected model for a later session.
              </p>
              <div className="preview-actions">
                {preview.canApply ? (
                  <button className="primary-button" type="button" disabled={busy || preview.changes.length === 0} onClick={applyPreview}>
                    Apply reviewed config
                  </button>
                ) : preview.exportOnly ? (
                  <span className="export-only-label">Export only · no files will be changed</span>
                ) : (
                  <span className="export-only-label">Nothing to apply</span>
                )}
              </div>
            </section>
          )}
        </>
      )}
    </main>
  );
}

function PreferenceRow({
  title,
  description,
  modelOptions,
  draft,
  saved,
  busy,
  onChange,
  onSave,
}: {
  title: string;
  description: string;
  modelOptions: ModelOption[];
  draft: Draft;
  saved?: Preference;
  busy: boolean;
  onChange: (field: keyof Draft, value: string) => void;
  onSave: () => void;
}) {
  return (
    <section className="selector-row" aria-label={title}>
      <div className="selector-heading">
        <h2>{title}</h2>
        <p>{description}</p>
      </div>
      <div className="selector-fields">
        <ModelFields label={title} modelOptions={modelOptions} draft={draft} onChange={onChange} />
        <button className="primary-button" type="button" disabled={busy || !draft.modelSlug || !draft.reasoningEffort} onClick={onSave}>
          Save preference
        </button>
      </div>
      {saved && <SavedState preference={saved} />}
    </section>
  );
}

function ModelFields({
  label,
  modelOptions,
  draft,
  onChange,
}: {
  label: string;
  modelOptions: ModelOption[];
  draft: Draft;
  onChange: (field: keyof Draft, value: string) => void;
}) {
  const selected = modelOptions.find((model) => model.modelSlug === draft.modelSlug);
  return (
    <>
      <label>
        <span>Model</span>
        <select aria-label={`${label} model`} value={draft.modelSlug} onChange={(event) => onChange("modelSlug", event.target.value)}>
          {modelOptions.map((model) => <option key={model.modelSlug} value={model.modelSlug}>{model.displayName} · {model.modelSlug}</option>)}
        </select>
      </label>
      <label>
        <span>Reasoning effort</span>
        <select aria-label={`${label} reasoning effort`} value={draft.reasoningEffort} onChange={(event) => onChange("reasoningEffort", event.target.value)}>
          {(selected?.reasoningEfforts ?? []).map((effort) => <option key={effort} value={effort}>{effort}</option>)}
        </select>
      </label>
    </>
  );
}

function SavedState({ preference }: { preference: Preference }) {
  return (
    <p className="saved-state">
      Saved: {preference.modelSlug} · {preference.reasoningEffort} · {preference.lastAppliedTarget ? `written to ${preference.lastAppliedTarget}` : "stored in Cathulu only"} · runtime effectiveness unverified
    </p>
  );
}
