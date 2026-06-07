import { motion } from "framer-motion";
import { GameIcon, type GameIconName } from "@/components/icons/GameIcon";
import { Badge } from "@/components/ui/badge";
import type { ClientInstallation } from "@/types";

export type ClientTypeId =
  | "qmclient"
  | "ddnet"
  | "ddnet-steam"
  | "qmclient-nightly"
  | "taterclient"
  | "bestclient"
  | "cactusclient"
  | "third-party";

export type ClientType = {
  id: ClientTypeId;
  name: string;
  subtitle: string;
  version: string;
  status: string;
  pathHint: string;
  icon: GameIconName;
  accent: string;
};

type GamesPanelProps = {
  clientTypes: ClientType[];
  selectedClientTypeId: ClientTypeId;
  selectedClient: ClientInstallation | null;
  clientPath: string;
  onSelectClientType: (id: ClientTypeId) => void;
};

function getPathText(client: ClientInstallation | null, clientPath: string, item: ClientType) {
  if (client?.install_dir && item.id === clientTypeIdFromClientId(client.client_id, client.install_dir)) {
    return client.install_dir;
  }

  if (item.id === "qmclient" && clientPath.trim()) {
    return clientPath;
  }

  return item.pathHint;
}

function clientTypeIdFromClientId(clientId: string, installDir: string): ClientTypeId {
  if (clientId === "ddnet_vanilla" && installDir.toLowerCase().includes("steamapps")) {
    return "ddnet-steam";
  }

  switch (clientId) {
    case "qmclient":
      return "qmclient";
    case "qmclient_nightly":
      return "qmclient-nightly";
    case "ddnet_vanilla":
      return "ddnet";
    case "taterclient":
      return "taterclient";
    case "bestclient":
      return "bestclient";
    case "cactusclient":
      return "cactusclient";
    default:
      return "third-party";
  }
}

export function GamesPanel(props: GamesPanelProps) {
  return (
    <section className="min-h-full">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="text-[11px] font-black tracking-[0.2em] text-[var(--dm-muted-ink)]">客户端列表</div>
          <h1 className="mt-2 text-4xl font-black tracking-[-0.06em] text-[var(--dm-ink)]">全部游戏</h1>
        </div>
        <div className="rounded-full border border-[var(--dm-border)] bg-white/70 px-4 py-2 text-xs font-black text-[var(--dm-muted-ink)] shadow-sm">
          {props.clientTypes.length} 个客户端
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {props.clientTypes.map((item, index) => {
          const active = item.id === props.selectedClientTypeId;
          const pathText = getPathText(props.selectedClient, props.clientPath, item);

          return (
            <motion.button
              key={item.id}
              type="button"
              initial={{ opacity: 0, y: 18 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: index * 0.04, duration: 0.28, ease: "easeOut" }}
              onClick={() => props.onSelectClientType(item.id)}
              className={`group relative min-h-[320px] overflow-hidden rounded-[34px] border p-4 text-left transition ${
                active
                  ? "border-[var(--dm-ink)] bg-white shadow-[0_26px_70px_rgba(47,52,64,0.16)]"
                  : "border-[var(--dm-border)] bg-white/72 shadow-[0_18px_44px_rgba(47,52,64,0.08)] hover:-translate-y-1 hover:bg-white/88"
              }`}
            >
              <div
                className="absolute inset-x-4 top-4 h-40 rounded-[28px]"
                style={{
                  background: `linear-gradient(135deg, ${item.accent} 0%, rgba(255,255,255,0.75) 78%)`
                }}
              />
              <div className="absolute right-6 top-8 text-white/80">
                <GameIcon name={item.icon} className="size-20" />
              </div>
              <div className="absolute inset-x-4 top-4 h-40 rounded-[28px] bg-[radial-gradient(circle_at_18%_18%,rgba(255,255,255,0.9),transparent_34%),linear-gradient(180deg,rgba(255,255,255,0),rgba(47,52,64,0.18))]" />

              <div className="relative z-10 flex h-full min-h-[288px] flex-col justify-between">
                <div className="flex justify-between gap-3">
                  <Badge className="border-0 bg-white/82 text-[10px] font-black text-[var(--dm-ink)]">
                    {item.version}
                  </Badge>
                  {active ? (
                    <Badge className="border-0 bg-[var(--dm-ink)] text-[10px] font-black text-white">当前</Badge>
                  ) : null}
                </div>

                <div>
                  <h2 className="text-2xl font-black tracking-[-0.045em] text-[var(--dm-ink)]">{item.name}</h2>
                  <div className="mt-1 text-sm font-bold text-[var(--dm-muted-ink)]">{item.subtitle}</div>

                  <div className="mt-5 grid gap-2">
                    <div className="flex items-center justify-between rounded-2xl bg-[var(--dm-soft)] px-3 py-2 text-xs font-black">
                      <span className="text-[var(--dm-muted-ink)]">状态</span>
                      <span className="text-[var(--dm-ink)]">{active ? "已选择" : item.status}</span>
                    </div>
                    <div className="rounded-2xl bg-[var(--dm-soft)] px-3 py-2">
                      <div className="text-[10px] font-black tracking-[0.14em] text-[var(--dm-muted-ink)]">路径</div>
                      <div className="mt-1 truncate text-xs font-bold text-[var(--dm-ink)]">{pathText}</div>
                    </div>
                  </div>
                </div>
              </div>
            </motion.button>
          );
        })}
      </div>
    </section>
  );
}
