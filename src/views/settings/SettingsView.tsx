import { useEffect, useRef, useState } from "react";
import { History, Info, Keyboard, Settings as Gear, ShieldCheck, Tag, WandSparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import { useStore } from "../../store";
import { Brand } from "../../components/Brand";
import { SwitchingPage } from "./SwitchingPage";
import { ExceptionsPage } from "./ExceptionsPage";
import { AmbiguousPage } from "./AmbiguousPage";
import { SnippetsPage } from "./SnippetsPage";
import { GeneralPage } from "./GeneralPage";
import { HistoryPage } from "./HistoryPage";
import { AboutPage, PrivacyPage } from "./InfoPages";

const SECTIONS = [
  { id: "switching", icon: Keyboard },
  { id: "exceptions", icon: Tag },
  { id: "snippets", icon: WandSparkles },
  { id: "history", icon: History },
  { id: "general", icon: Gear },
  { id: "privacy", icon: ShieldCheck },
  { id: "about", icon: Info },
] as const;
type Page = (typeof SECTIONS)[number]["id"] | "ambiguous";

function StatusLine() {
  const { t, i18n } = useTranslation();
  const status = useStore((s) => s.status);
  if (!status) return null;
  const paused = status.pausedUntil && status.pausedUntil * 1000 > Date.now();
  const time = paused ? new Date(status.pausedUntil! * 1000).toLocaleTimeString(i18n.language, { hour: "2-digit", minute: "2-digit" }) : "";
  const [text, live] = paused ? [t("status.paused", { time }), false]
    : !status.running ? [t("status.down"), false]
    : !status.autoEnabled ? [t("status.autoOff"), false]
    : [t("status.working"), true];
  return (
    <div className="px-3 pb-4 text-[12px]">
      <div className="flex items-center gap-2 text-fg/75">
        <span className={`size-2 shrink-0 rounded-full ${live ? "bg-detail shadow-[0_0_0_3px] shadow-detail/20" : "bg-fg/35"}`} />
        <span>{text}</span>
      </div>
      {paused && (
        <button type="button" className="mt-1 ml-4 cursor-pointer text-detail underline-offset-2 hover:underline"
          onClick={() => ipc.resume().then((s) => useStore.setState({ status: s })).catch(console.error)}>
          {t("status.resume")}
        </button>
      )}
    </div>
  );
}

export function SettingsView() {
  const { t } = useTranslation();
  const [page, setPage] = useState<Page>("switching");
  const scroller = useRef<HTMLDivElement>(null);
  useEffect(() => { scroller.current?.scrollTo(0, 0); }, [page]);
  const active = page === "ambiguous" ? "exceptions" : page;

  return (
    <div className="flex h-full">
      <nav aria-label="Keygloss" className="flex w-[220px] shrink-0 flex-col border-r border-fg/[0.08] bg-fg/[0.03]">
        <div className="flex items-center gap-2.5 px-4 pt-5 pb-4">
          <Brand size={28} />
          <span className="text-[15px] font-semibold tracking-tight">Keygloss</span>
        </div>
        <ul className="flex flex-1 flex-col gap-0.5 px-2">
          {SECTIONS.map(({ id, icon: Icon }) => (
            <li key={id}>
              <button type="button" aria-current={active === id ? "page" : undefined} onClick={() => setPage(id)}
                className={`relative flex h-8 w-full cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-left transition-colors ${
                  active === id ? "bg-fg/[0.08] font-semibold" : "text-fg/80 hover:bg-fg/[0.05] hover:text-fg"}`}>
                {active === id && <span className="absolute top-1.5 bottom-1.5 left-0 w-[3px] rounded-full bg-accent" />}
                <Icon size={16} strokeWidth={active === id ? 2.2 : 1.8} className={active === id ? "text-accent" : "text-fg/65"} />
                {t(`nav.${id}`)}
              </button>
            </li>
          ))}
        </ul>
        <StatusLine />
      </nav>
      <main ref={scroller} className="flex-1 overflow-x-hidden overflow-y-auto">
        <div key={page} className="kg-in ml-6 w-[600px] pt-7 pb-10">
          {page === "switching" && <SwitchingPage />}
          {page === "exceptions" && <ExceptionsPage onOpenAmbiguous={() => setPage("ambiguous")} />}
          {page === "ambiguous" && <AmbiguousPage onBack={() => setPage("exceptions")} />}
          {page === "snippets" && <SnippetsPage />}
          {page === "history" && <HistoryPage />}
          {page === "general" && <GeneralPage />}
          {page === "privacy" && <PrivacyPage />}
          {page === "about" && <AboutPage />}
        </div>
      </main>
    </div>
  );
}
