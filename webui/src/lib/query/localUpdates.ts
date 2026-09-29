import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getLocalAppUpdateStatus, runLocalAppUpdate } from "../api/updates";
export function useLocalAppUpdate() {
  const qc = useQueryClient();
  const key = ["local-app-update"];
  const status = useQuery({ queryKey: key, queryFn: getLocalAppUpdateStatus, refetchInterval: 30000 });
  const action = useMutation({ mutationFn: runLocalAppUpdate, onSuccess: () => qc.invalidateQueries({ queryKey: key }) });
  return { status, action };
}
