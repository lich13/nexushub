import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";
import {
  contractDtoCatalog,
  contractHostSurfaces,
  contractVisual,
  contractCapabilitiesByHostSurface,
  sharedContractActions
} from "./contractRegistry";
import { contractDtoNameSet, contractDtoNames } from "./contractDtoMap";
import {
  desktopLanForbiddenVisualSurfaces,
  desktopTauriOnlyVisualSurfaces,
  linuxWebOnlyVisualSurfaces,
  macosForbiddenVisualSurfaces,
  sharedActionLabels,
  sharedCorePanelTitles,
  sharedDisabledStates,
  sharedNavigationLabels
} from "./visualContract";

type ContractAction = {
  id: string;
  kind: "rpc" | "transport";
  scope: "shared" | "host_only" | "transport";
  linuxRpc?: string;
  tauriCommand?: string;
  webuiWrapper?: string;
  dtoOwner?: string;
  requestDto?: string;
  responseDto?: string;
  hostOnlyReason?: string;
};

type NexusHubContract = {
  hostSurfaces: string[];
  visual: {
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
  actions: ContractAction[];
  dtoCatalog: Record<string, { core: string; webui: string }>;
};

function contract(): NexusHubContract {
  const url = new URL("../../../../contracts/nexushub-contract.json", import.meta.url);
  return JSON.parse(readFileSync(fileURLToPath(url), "utf8")) as NexusHubContract;
}

function repositoryFile(path: string): string {
  const url = new URL(`../../../../${path}`, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf8");
}

const apiSources = import.meta.glob([
  "../api/**/*.ts",
  "!../api/**/*.test.ts"
], {
  eager: true,
  query: "?raw",
  import: "default"
}) as Record<string, string>;

function apiCommandLiterals(): Set<string> {
  const out = new Set<string>();
  for (const source of Object.values(apiSources)) {
    for (const pattern of [
      /\bcallCommand(?:<[^"']+>)?\(\s*["']([^"']+)["']/g,
      /\bstartProbeCommand\(\s*["']([^"']+)["']/g,
      /\brunTypedUpdateCommand\(\s*["']([^"']+)["']/g,
    ]) {
      for (const match of source.matchAll(pattern)) {
        out.add(match[1]);
      }
    }
    if (source.includes("uploadFilesTransport")) out.add("uploadFiles");
    if (source.includes("openThreadEventStream")) out.add("threadEvents");
  }
  return out;
}

describe("contract registry", () => {
  test("has a schema and architecture audit that describe the supported parity boundary", () => {
    const schema = JSON.parse(repositoryFile("contracts/nexushub-contract.schema.json")) as {
      $id?: string;
      required?: string[];
      properties?: Record<string, unknown>;
    };
    expect(schema.$id).toBe("https://github.com/lich13/nexushub/contracts/nexushub-contract.schema.json");
    expect(schema.required).toEqual([
      "schemaVersion",
      "hostSurfaces",
      "capabilities",
      "capabilitiesByHostSurface",
      "visual",
      "actions",
      "dtoCatalog"
    ]);
    expect(Object.keys(schema.properties ?? {})).toEqual([
      "schemaVersion",
      "hostSurfaces",
      "capabilities",
      "capabilitiesByHostSurface",
      "visual",
      "actions",
      "dtoCatalog"
    ]);

    const audit = repositoryFile("docs/analysis/cc-switch-architecture-parity.md");
    for (const required of [
      "cc-switch origin/main",
      "cc-switch feat/webd",
      "NexusHub v0.1.144",
      "Windows desktop",
      "Linux arm64",
      "nexushub-webd-linux-x86_64.tar.gz",
      "NexusHub-*-Linux-x86_64.AppImage",
      "contracts/nexushub-contract.json"
    ]) {
      expect(audit, `audit missing ${required}`).toContain(required);
    }
  });

  test("exports registry-derived host surfaces, visual copy, and capabilities without test-only file reads", () => {
    expect(contractHostSurfaces).toEqual(contract().hostSurfaces);
    expect(contractVisual.navigation).toEqual(contract().visual.navigation);
    expect(contractVisual.corePanelTitles).toEqual(contract().visual.corePanelTitles);
    expect(contractVisual.actionLabels).toEqual(contract().visual.actionLabels);
    expect(contractVisual.disabledStates).toEqual(contract().visual.disabledStates);
    expect(contractVisual.forbidden.desktopLanWebui).toEqual(contract().visual.forbidden.desktopLanWebui);
    expect(contractDtoCatalog).toEqual(contract().dtoCatalog);
    expect(contractCapabilitiesByHostSurface.desktop_embedded_tauri).toEqual(
      contract().capabilitiesByHostSurface.desktop_embedded_tauri
    );
    expect(sharedContractActions.map((action) => action.id)).toEqual(
      contract().actions.filter((action) => action.scope === "shared").map((action) => action.id)
    );
  });

  test("shared and host-only action declarations are complete enough to prevent one-sided adapters", () => {
    for (const action of contract().actions) {
      if (action.scope === "shared") {
        expect(action.coreUseCase, `${action.id} coreUseCase`).toBeTruthy();
        expect(action.linuxRpc, `${action.id} linuxRpc`).toBeTruthy();
        expect(action.tauriCommand, `${action.id} tauriCommand`).toBeTruthy();
        expect(action.webuiWrapper, `${action.id} webuiWrapper`).toBeTruthy();
        expect(action.dtoOwner, `${action.id} dtoOwner`).toBeTruthy();
        expect(action.requestDto, `${action.id} requestDto`).toBeTruthy();
        expect(action.responseDto, `${action.id} responseDto`).toBeTruthy();
      }
      if (action.scope === "transport") {
        expect(action.webuiWrapper, `${action.id} webuiWrapper`).toBeTruthy();
        expect(action.dtoOwner, `${action.id} dtoOwner`).toBeTruthy();
        expect(action.requestDto, `${action.id} requestDto`).toBeTruthy();
        expect(action.responseDto, `${action.id} responseDto`).toBeTruthy();
      }
      if (action.scope === "host_only") {
        expect(action.hostOnlyReason, `${action.id} hostOnlyReason`).toBeTruthy();
      }
    }
  });

  test("shared and transport DTO declarations resolve through the WebUI DTO marker map", () => {
    const dtoCatalog = contract().dtoCatalog;
    expect(new Set(contractDtoNames)).toEqual(new Set(Object.values(dtoCatalog).map((entry) => entry.webui)));
    for (const action of contract().actions) {
      if (action.scope !== "shared" && action.scope !== "transport") continue;
      for (const key of ["requestDto", "responseDto"] as const) {
        const dtoName = action[key];
        expect(dtoName, `${action.id} ${key}`).toBeTruthy();
        expect(dtoCatalog[dtoName ?? ""], `${action.id} ${key} catalog entry`).toBeTruthy();
        expect(contractDtoNameSet.has(dtoCatalog[dtoName ?? ""].webui), `${action.id} ${key} WebUI DTO marker`).toBe(true);
      }
    }
  });

  test("declares the shared visual vocabulary used by both WebUI and Tauri shells", () => {
    const visual = contract().visual;
    expect(visual.navigation).toEqual([...sharedNavigationLabels]);
    expect(visual.corePanelTitles).toEqual([...sharedCorePanelTitles]);
    expect(visual.actionLabels).toEqual(sharedActionLabels);
    expect(visual.disabledStates).toEqual(sharedDisabledStates);
    expect(visual.linuxWebOnly).toEqual([...linuxWebOnlyVisualSurfaces]);
    expect(visual.desktopTauriOnly).toEqual([...desktopTauriOnlyVisualSurfaces]);
    expect(visual.forbidden.desktopEmbeddedTauri).toEqual([...macosForbiddenVisualSurfaces]);
    expect(visual.forbidden.desktopLanWebui).toEqual([...desktopLanForbiddenVisualSurfaces]);
  });

  test("covers WebUI runtime command wrappers without ad hoc command strings", () => {
    const commandLiterals = apiCommandLiterals();
    const contractCommands = new Set(
      contract().actions
        .filter((action) => typeof action.webuiWrapper === "string" && action.webuiWrapper.trim().length > 0)
        .flatMap((action) => [action.linuxRpc, action.id])
        .filter((value): value is string => typeof value === "string" && value.length > 0)
    );

    expect(commandLiterals).toEqual(contractCommands);
  });

  test("keeps host surface names explicit for runtime capability policy", () => {
    expect(contract().hostSurfaces).toEqual([
      "linux_server_webui",
      "desktop_embedded_tauri",
      "desktop_lan_webui"
    ]);
  });
});
