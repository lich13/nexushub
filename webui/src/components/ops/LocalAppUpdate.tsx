import { RefreshCw } from "lucide-react";
import { Panel } from "../common/Panel";
import { useLocalAppUpdate } from "../../lib/query/localUpdates";
export function LocalAppUpdate() {
  const { status, action } = useLocalAppUpdate();
  return <Panel title="本机 App 更新" icon={<RefreshCw size={18} />}>
    <div className="button-row"><span>{status.data?.current_version}</span>
      <button className="secondary-button" disabled={action.isPending} onClick={() => action.mutate("check")}>检查更新</button>
      <button className="primary-button" disabled={action.isPending || !status.data?.update_available} onClick={() => action.mutate("install")}>安装 {status.data?.latest_version}</button>
    </div>
    {(status.error || action.error) && <div role="alert" className="form-error">{(status.error ?? action.error)?.message}</div>}
  </Panel>;
}
