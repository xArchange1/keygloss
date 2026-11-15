import { defineConfig } from "astro/config";

// Served from https://xarchange1.github.io/keygloss/. Asset paths in the page are relative,
// so moving to keygloss.app only needs `base` removed and `site` changed.
export default defineConfig({
  site: "https://xarchange1.github.io",
  base: "/keygloss",
});
