# Keygloss

Windows and macOS Tauri rewrite of keyboop (macOS-only, Swift, MIT). Read `docs/PLAN.md` first: rules, layout, design tokens, how to build and test.

- The original is cloned at `C:\dev\keygloss\reference\original-app` (https://github.com/iffuno/keyboop). It is gitignored, so it is missing from worktrees: read it at that absolute path. Never edit it.
- The original is MIT licensed: keep its copyright notice and license text in this project's licensing/attribution.
- After filing a PR, queue it with `gh pr merge --auto --merge` and move on: do not wait for CI. Run the checks in `docs/PLAN.md` locally first.
- Admins can bypass the `main` ruleset. Push straight to `main` only when asked to; otherwise file a PR.
