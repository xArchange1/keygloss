import { Keyboard } from "lucide-react";
import { useTranslation } from "react-i18next";

export default function App() {
  const { t } = useTranslation();
  return (
    <main className="flex h-full flex-col items-center justify-center gap-3">
      <Keyboard className="text-accent" size={40} />
      <h1 className="text-2xl font-semibold">
        Key<span className="text-accent">gloss</span>
      </h1>
      <p className="flex items-center gap-2 opacity-80">
        <span className="inline-block size-2 rounded-full bg-detail" />
        {t("placeholder")}
      </p>
    </main>
  );
}
