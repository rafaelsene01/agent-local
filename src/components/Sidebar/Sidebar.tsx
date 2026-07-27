import { useTranslation } from "react-i18next";
import { ChatList } from "./ChatList";
import { DocumentsSection } from "./DocumentsSection";
import { RuntimeSection } from "./RuntimeSection";
import { SettingsSection } from "./SettingsSection";

export function Sidebar() {
  const { t } = useTranslation();

  return (
    <aside className="flex h-full w-72 shrink-0 flex-col bg-[var(--bg-sidebar)] text-[var(--text-primary)]">
      <div className="flex items-center gap-2 px-3 py-3">
        <div className="h-6 w-6 rounded-md bg-[var(--accent)]" />
        <span className="text-sm font-semibold">{t("app.name")}</span>
      </div>

      <ChatList />
      <DocumentsSection />
      <RuntimeSection />
      <SettingsSection />
    </aside>
  );
}
