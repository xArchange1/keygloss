// Prints the notes of release <version> from src/changelog.ts: the English list, RU_MARKER, the
// Russian list. They become the GitHub release body and `notes` in latest.json, which the
// update dialog shows. Nothing is printed for a version with no entry.
// Usage: node --experimental-strip-types scripts/release-notes.ts 0.2.0
import { CHANGELOG } from "../src/changelog.ts";
import { RU_MARKER } from "../src/lib/releaseNotes.ts";

const release = CHANGELOG.find((r) => r.version === process.argv[2]);
const list = (lines: string[]) => lines.map((l) => `- ${l}`).join("\n");
if (release) {
  const ru = `<details><summary>Русский</summary>\n\n${list(release.ru)}\n\n</details>`;
  console.log(`${list(release.en)}\n\n${RU_MARKER}\n${ru}`);
}
