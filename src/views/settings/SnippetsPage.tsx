import { useTranslation } from "react-i18next";
import { useSettings, useUpdate } from "../../store";
import { PairEditor } from "../../components/PairEditor";
import { BlockTitle, Button, Card, CheckPill, Hint, PageTitle, SectionTitle } from "../../components/ui";
import { OptionalHotkey } from "./SwitchingPage";

export function SnippetsPage() {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const allOff = !s.snippetExpandSpace && !s.snippetExpandEnter && !s.snippetExpandTab;
  const autoreplace = s.snippetsOrdered.filter(([a]) => a);
  const showCopy = s.textSnippets.length === 0 && autoreplace.length > 0;

  // "Copy from autoreplace" merges by name: entries whose name already exists are skipped.
  const copyFromAutoreplace = () => {
    const names = new Set(s.textSnippets.map(([n]) => n));
    update({ textSnippets: [...s.textSnippets, ...autoreplace.filter(([n]) => !names.has(n))] });
  };

  return (
    <>
      <PageTitle title={t("snip.title")} sub={t("snip.sub")} />
      <PairEditor pairs={s.snippetsOrdered} onChange={(snippetsOrdered) => update({ snippetsOrdered })}
        cols={[t("snip.colTrigger"), t("snip.colExpansion")]} placeholders={[t("snip.phTrigger"), t("snip.phExpansion")]} />

      <SectionTitle>{t("snip.expandOn")}</SectionTitle>
      <div className="flex gap-1.5 px-1">
        <CheckPill checked={s.snippetExpandSpace} onChange={(v) => update({ snippetExpandSpace: v })}>{t("keys.space")}</CheckPill>
        <CheckPill checked={s.snippetExpandEnter} onChange={(v) => update({ snippetExpandEnter: v })}>{t("keys.enter")}</CheckPill>
        <CheckPill checked={s.snippetExpandTab} onChange={(v) => update({ snippetExpandTab: v })}>{t("keys.tab")}</CheckPill>
      </div>
      {allOff && <Hint tone="warn">{t("snip.disabled")}</Hint>}
      <Hint>{t("snip.hint")}</Hint>

      <BlockTitle title={t("snip.textsTitle")} help={t("snip.textsSub")} />
      <Card>
        <OptionalHotkey title={t("snip.pickOn")} help={t("snip.pickHint")} target="snippetPick" field="snippetPickHotkey" />
      </Card>
      <div className="mt-2.5">
        <PairEditor pairs={s.textSnippets} onChange={(textSnippets) => update({ textSnippets })}
          cols={[t("snip.colName"), t("snip.colText")]} placeholders={[t("snip.phName"), t("snip.phText")]} />
      </div>
      {showCopy && (
        <div className="mt-2 flex items-start justify-between gap-4">
          <Hint>{t("snip.copyFromHint")}</Hint>
          <Button className="mt-1.5" onClick={copyFromAutoreplace}>{t("snip.copyFrom")}</Button>
        </div>
      )}
    </>
  );
}
