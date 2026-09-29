import { invokeNative } from "../runtime";

export type PlanSaveRequest = { filename: string; markdown: string };
export type PlanSaveResult = { filename: string };

export function savePlanMarkdown(request: PlanSaveRequest): Promise<PlanSaveResult> {
  return invokeNative<PlanSaveResult>("plans.save", { request });
}
