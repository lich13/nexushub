import registry from "../../../../contracts/nexushub-contract.json";
import type { HostSurface } from "../../types";

export type ContractActionScope = "shared" | "host_only" | "transport";
export type ContractActionKind = "rpc" | "transport";

export type ContractAction = {
  id: string;
  kind: ContractActionKind;
  scope: ContractActionScope;
  coreUseCase?: string;
  linuxRpc?: string;
  tauriCommand?: string;
  webuiWrapper?: string;
  hostOnlyReason?: string;
};

export type ContractVisual = {
  navigation: string[];
  corePanelTitles: string[];
  actionLabels: Record<string, string>;
  disabledStates: Record<string, string>;
  linuxWebOnly: string[];
  desktopTauriOnly: string[];
  forbidden: {
    desktopEmbeddedTauri: string[];
    desktopLanWebui: string[];
  };
};

export type NexusHubContractRegistry = {
  schemaVersion: number;
  hostSurfaces: HostSurface[];
  capabilities: string[];
  capabilitiesByHostSurface: Record<HostSurface, string[]>;
  visual: ContractVisual;
  actions: ContractAction[];
};

export const contractRegistry = registry as NexusHubContractRegistry;

export const contractHostSurfaces = contractRegistry.hostSurfaces;
export const contractCapabilities = contractRegistry.capabilities;
export const contractCapabilitiesByHostSurface = contractRegistry.capabilitiesByHostSurface;
export const contractVisual = contractRegistry.visual;
export const contractActions = contractRegistry.actions;
export const sharedContractActions = contractActions.filter((action) => action.scope === "shared");
export const hostOnlyContractActions = contractActions.filter((action) => action.scope === "host_only");
