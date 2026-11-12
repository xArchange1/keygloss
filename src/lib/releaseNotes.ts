/** Splits a release's notes (scripts/release-notes.ts) into the English and the Russian half. */
export const RU_MARKER = "<!-- ru -->";

/** The lines of release notes `body` for the interface language, without list markers. */
export function notesFor(body: string, lang: "en" | "ru"): string[] {
  const [en, ru] = body.split(RU_MARKER);
  return ((lang === "ru" && ru) || en).split("\n").map((l) => l.trim().replace(/^[-*] /, "")).filter(Boolean);
}
