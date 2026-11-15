# Homebrew cask for Keygloss: brew install --cask xArchange1/tap/keygloss
# Each stable release copies it to Casks/keygloss.rb in github.com/xArchange1/homebrew-tap with
# `version` and `sha256` set (.github/workflows/release.yml), so edit it here, not in the tap.
cask "keygloss" do
  version "0.1.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/xArchange1/keygloss/releases/download/v#{version}/Keygloss_#{version}_universal.dmg"
  name "Keygloss"
  desc "Fixes words typed in the wrong keyboard layout (Russian/English)"
  homepage "https://github.com/xArchange1/keygloss"

  livecheck do
    url :url
    strategy :github_latest
  end

  # The app updates itself.
  auto_updates true
  depends_on macos: :big_sur

  app "Keygloss.app"

  # Without a Developer ID the app is not notarized: Gatekeeper would block its first launch.
  postflight_steps do
    run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "{{appdir}}/Keygloss.app"]
  end

  uninstall quit:      "com.keygloss.app",
            launchctl: "Keygloss"

  zap trash: [
    "~/Library/Application Support/com.keygloss.app",
    "~/Library/Caches/com.keygloss.app",
    "~/Library/LaunchAgents/Keygloss.plist",
    "~/Library/Logs/com.keygloss.app",
    "~/Library/WebKit/com.keygloss.app",
  ]
end
