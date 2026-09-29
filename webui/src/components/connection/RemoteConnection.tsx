import { Cloud, Monitor } from "lucide-react";
import { useState } from "react";
import { remoteRemove, remoteSave, remoteSelect, useConnection, type MachineTarget } from "../../lib/query/connection";

export function MachineButton({ collapsed = false, onConfigure }: { collapsed?: boolean; onConfigure: () => void }) {
  const connection = useConnection();
  const [error, setError] = useState("");
  return <div className="machine-control">
    <label className={`machine-button ${collapsed ? "compact" : ""}`} title={connection.target === "local" ? "本机" : "腾讯云"}>
      {connection.target === "local" ? <Monitor size={17} /> : <Cloud size={17} />}
      {!collapsed && <span>{connection.target === "local" ? "本机" : "腾讯云"}</span>}
      <select aria-label="当前机器" value={connection.target} disabled={Boolean(connection.writes || connection.changing)} onChange={async event => {
        const target = event.target.value as MachineTarget;
        if (target === "remote" && !connection.configured) { onConfigure(); return; }
        setError("");
        try { await remoteSelect(target); } catch (error) { setError((error as Error).message); }
      }}>
        <option value="local">本机</option><option value="remote">腾讯云</option>
      </select>
    </label>
    {error && <span role="alert" className="form-error">{error}</span>}
  </div>;
}

export function RemoteConnectionSettings() {
  const connection = useConnection();
  const [baseUrl, setBaseUrl] = useState(connection.baseUrl ?? "");
  const [apiKey, setApiKey] = useState("");
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const busy = Boolean(connection.writes || connection.changing);
  return <form className="panel remote-connection-form" onSubmit={async event => {
    event.preventDefault(); setError(""); setSaved(false);
    try { await remoteSave({ revision: connection.revision, baseUrl, apiKey }); setApiKey(""); setSaved(true); }
    catch (error) { setError((error as Error).message); }
  }}>
    <label>HTTPS 地址<input type="url" required value={baseUrl} onChange={event => setBaseUrl(event.target.value)} autoComplete="off" disabled={busy} placeholder="https://api.example.com/nexushub/" /></label>
    <label>管理员 API Key<input type="password" required value={apiKey} onChange={event => setApiKey(event.target.value)} autoComplete="off" disabled={busy} /></label>
    <div className="button-row">
      <button className="primary-button" disabled={busy || !baseUrl || !apiKey}>{connection.changing ? "连接中…" : "验证并保存"}</button>
      {connection.configured && <button type="button" className="danger-button soft" disabled={busy} onClick={async () => {
        setError("");
        try { await remoteRemove(); setApiKey(""); setBaseUrl(""); } catch (error) { setError((error as Error).message); }
      }}>移除连接</button>}
    </div>
    {(saved || connection.notice) && <span role="status">已保存</span>}
    {error && <div className="form-error" role="alert">{error}</div>}
  </form>;
}
