import type {
  IpcBackend,
  ServiceMutationReport,
  ServiceView,
} from "./ipc-types";

// 契约 §20（服务总控）mock：同签名独立文件（mock-authz 先例）。内存态模拟
// `<data-dir>/services.json` 真值源：闭集 upsert、双读回落、损坏显式报错。
// 闭集表与 crates/p2p-service registry 常量逐字一致（§20.4-6）；视图层服务
// 清单一律经 servicesList 枚举，前端禁硬编码（本表仅 mock 后端模拟用）。

type ServiceKind = ServiceView["kind"];

interface ClosedSetEntry {
  serviceId: string;
  kind: ServiceKind;
  // 收编型默认走双读回落，不取本字段（§20.4-3）。
  defaultEnabled: boolean;
}

const CLOSED_SET: ReadonlyArray<ClosedSetEntry> = [
  { serviceId: "serve.llm_share", kind: "boolean", defaultEnabled: false },
  { serviceId: "serve.tunnel", kind: "boolean", defaultEnabled: false },
  { serviceId: "serve.a2a", kind: "explicit", defaultEnabled: true },
  { serviceId: "serve.acp", kind: "explicit", defaultEnabled: true },
  { serviceId: "net.rendezvous_register", kind: "explicit", defaultEnabled: true },
  { serviceId: "net.relay", kind: "explicit", defaultEnabled: true },
  { serviceId: "net.observe", kind: "explicit", defaultEnabled: true },
  { serviceId: "serve.rendezvous_server", kind: "explicit", defaultEnabled: true },
  { serviceId: "discovery.mdns", kind: "adopted", defaultEnabled: true },
  { serviceId: "net.lan_only", kind: "adopted", defaultEnabled: false },
];

const CORRUPT_ERROR =
  "services.json 损坏或版本不符，服务总控已拒绝读取；请修复或删除该文件后重试";

export interface MockServicesDeps {
  isRunning: () => boolean;
  // 收编型双读回落（§20.4-3）：无条目时分别回落 GuiConfig 对应字段。
  enableMdns: () => boolean;
  lanOnly: () => boolean;
}

export function createMockServices(deps: MockServicesDeps) {
  const entries = new Map<string, boolean>();
  let corrupt = false;

  function requireReadable(): void {
    if (corrupt) throw new Error(CORRUPT_ERROR);
  }

  function fallbackEnabled(entry: ClosedSetEntry): boolean {
    if (entry.serviceId === "discovery.mdns") return deps.enableMdns();
    if (entry.serviceId === "net.lan_only") return deps.lanOnly();
    return entry.defaultEnabled;
  }

  function findEntry(serviceId: string): ClosedSetEntry | undefined {
    return CLOSED_SET.find((entry) => entry.serviceId === serviceId);
  }

  function requireClosedSet(serviceId: string): ClosedSetEntry {
    const entry = findEntry(serviceId);
    if (!entry) {
      throw new Error(
        `serviceId 不在服务闭集: ${serviceId}（闭集: ${CLOSED_SET.map((s) => s.serviceId).join(", ")}）`,
      );
    }
    return entry;
  }

  function viewOf(entry: ClosedSetEntry): ServiceView {
    return {
      serviceId: entry.serviceId,
      kind: entry.kind,
      enabled: entries.get(entry.serviceId) ?? fallbackEnabled(entry),
      requiresRestart: deps.isRunning(),
    };
  }

  const backend: Pick<IpcBackend, "servicesList" | "servicesSetEnabled"> = {
    async servicesList() {
      await delay(60);
      requireReadable();
      return { services: CLOSED_SET.map(viewOf) };
    },

    async servicesSetEnabled(
      serviceId,
      enabled,
    ): Promise<ServiceMutationReport> {
      await delay(120);
      requireReadable();
      const entry = requireClosedSet(serviceId);
      entries.set(entry.serviceId, enabled);
      return {
        serviceId: entry.serviceId,
        enabled,
        requiresRestart: deps.isRunning(),
      };
    },
  };

  // dev 注入入口：预置条目/模拟存储损坏/复位（window.__MOCK_SERVICES__）。
  const controller = {
    seed(serviceId: string, enabled: boolean): void {
      entries.set(requireClosedSet(serviceId).serviceId, enabled);
    },
    setCorrupt(next: boolean): void {
      corrupt = next;
    },
    reset(): void {
      entries.clear();
      corrupt = false;
    },
  };

  return { backend, controller };
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}
