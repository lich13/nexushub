import { ChevronRight } from "lucide-react";
import {
  jobFailureAnalysisView,
  jobOutputView,
  type RuntimeCapabilityInput
} from "../../lib/domain/runtimeViewModel";
import type { JobRecord } from "../../types";

export function JobList({ jobs, capabilities }: { jobs: JobRecord[]; capabilities: RuntimeCapabilityInput }) {
  return (
    <div className="job-list">
      {jobs.map((job) => {
        const analysis = job.failure_analysis ? jobFailureAnalysisView(job.failure_analysis, capabilities) : null;
        return (
          <details key={job.id} className="job-item">
            <summary>
              <span>{job.title}</span>
              <StatusDot status={job.status} />
              <ChevronRight size={16} />
            </summary>
            <div className="job-meta">
              <span>{job.kind}</span>
              {job.thread_id && <span>{job.thread_id}</span>}
              {job.turn_id && <span>{job.turn_id}</span>}
            </div>
            {analysis && (
              <div className="job-analysis">
                <strong>{analysis.label}</strong>
                <p>{analysis.explanation}</p>
                <ul>
                  {analysis.suggestions.map((suggestion) => <li key={suggestion}>{suggestion}</li>)}
                </ul>
              </div>
            )}
            <pre>{jobOutputView(job.output || job.error || "no output", capabilities)}</pre>
          </details>
        );
      })}
      {jobs.length === 0 && <div className="muted-row">暂无后台 job</div>}
    </div>
  );
}

function StatusDot({ status }: { status: string }) {
  const label = ({ succeeded: "成功", failed: "失败", running: "进行中", queued: "排队中", cancelled: "已取消" } as Record<string, string>)[status] ?? status;
  return <><span className={`status-dot ${status}`} aria-hidden="true" /><small>{label}</small></>;
}
