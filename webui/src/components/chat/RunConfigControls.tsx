import { CheckCircle2, ClipboardCheck, Lock, Plus, RefreshCw, ShieldCheck, SlidersHorizontal } from "lucide-react";
import { ReactNode } from "react";
import {
  applyPermissionPreset,
  modelSupportsServiceTier,
  reasoningOptions,
  type PermissionPresetId,
  type RunConfig
} from "../../lib/domain/codexViewModel";
import { planModeButtonState } from "../../lib/domain/conversationViewModel";
import type { CodexModel, PermissionProfile, ThreadStatus } from "../../types";

const permissionPresets: Array<{ id: PermissionPresetId; label: string; description: string; icon: ReactNode }> = [
  { id: "ask", label: "请求批准", description: "编辑外部文件和使用互联网时始终询问", icon: <Lock size={17} /> },
  { id: "auto", label: "替我审批", description: "仅对检测到的风险操作请求批准", icon: <ShieldCheck size={17} /> },
  { id: "full", label: "完全访问权限", description: "可不受限制地访问互联网和文件", icon: <CheckCircle2 size={17} /> },
  { id: "custom", label: "自定义 (config.toml)", description: "使用 config.toml 中定义的权限", icon: <SlidersHorizontal size={17} /> }
];

export function RunConfigControls({ config, setConfig, models, unavailable, onPickFiles, uploadInProgress = false, threadStatus, hasPendingPlan = false, hasPendingQuestion = false }: {
  config: RunConfig;
  setConfig: (config: RunConfig) => void;
  models: CodexModel[];
  profiles: PermissionProfile[];
  unavailable: { models?: boolean; profiles?: boolean; config?: boolean };
  onPickFiles?: () => void;
  uploadInProgress?: boolean;
  threadStatus?: ThreadStatus | string;
  hasPendingPlan?: boolean;
  hasPendingQuestion?: boolean;
}) {
  const modelList = models.some((item) => item.id === config.model)
    ? models
    : config.model
      ? [{ id: config.model, label: config.model }, ...models]
      : models;
  const activePreset = permissionPresets.find((item) => item.id === config.permissionPreset) ?? permissionPresets[2];
  const supportsFast = modelSupportsServiceTier(modelList, config.model, "priority");
  const serviceTier = supportsFast ? config.serviceTier : "";
  const planButton = planModeButtonState(config.collaborationMode === "plan", threadStatus, hasPendingPlan, hasPendingQuestion);
  return (
    <div className="composer-config">
      <div className="composer-toolbar">
        <button
          type="button"
          className="composer-chip icon-only"
          title={uploadInProgress ? "附件上传中" : "上传本地文件"}
          onClick={onPickFiles}
          disabled={!onPickFiles || uploadInProgress}
        >
          <Plus size={15} />
        </button>
        {supportsFast && (
          <button
            type="button"
            className={serviceTier === "priority" ? "composer-chip active" : "composer-chip"}
            onClick={() => setConfig({ ...config, serviceTier: serviceTier === "priority" ? "" : "priority" })}
            title="使用 Codex priority service tier"
          >
            <RefreshCw size={15} />Fast
          </button>
        )}
        <button
          type="button"
          className={planButton.pressed ? "composer-chip active" : "composer-chip"}
          aria-pressed={planButton.pressed}
          title={planButton.statusText}
          onClick={() => setConfig({ ...config, collaborationMode: config.collaborationMode === "plan" ? "" : "plan" })}
        >
          <ClipboardCheck size={15} />{planButton.label}
        </button>
        <span className="composer-chip muted">{planButton.statusText}</span>
        <label className="permission-menu-trigger">
          <ShieldCheck size={15} />
          <select value={config.permissionPreset} onChange={(event) => setConfig(applyPermissionPreset(config, event.target.value as PermissionPresetId))}>
            {permissionPresets.map((preset) => <option key={preset.id} value={preset.id}>{preset.label}</option>)}
          </select>
        </label>
      </div>
      <div className="composer-grid main-config">
        <label>
          <span>模型</span>
          {modelList.length > 0 ? (
            <select value={config.model} onChange={(event) => {
              const model = event.target.value;
              setConfig({
                ...config,
                model,
                serviceTier: modelSupportsServiceTier(modelList, model, "priority") ? config.serviceTier : ""
              });
            }}>
              {modelList.map((item) => <option key={item.id} value={item.id}>{item.label ?? item.id}</option>)}
            </select>
          ) : (
            <input value={config.model} onChange={(event) => setConfig({ ...config, model: event.target.value })} placeholder={unavailable.models ? "模型接口不可用" : "model"} />
          )}
        </label>
        <label>
          <span>Reasoning</span>
          <select value={config.reasoning} onChange={(event) => setConfig({ ...config, reasoning: event.target.value })}>
            {reasoningOptions.map((value) => <option key={value || "default"} value={value}>{value || "default"}</option>)}
          </select>
        </label>
      </div>
      <div className="permission-summary">
        <div className="permission-summary-icon">{activePreset.icon}</div>
        <div>
          <strong>{activePreset.label}</strong>
          <span>{activePreset.description}</span>
        </div>
      </div>
      {unavailable.config && <div className="config-note">Codex 默认配置接口不可用，使用当前表单值发送。</div>}
    </div>
  );
}
