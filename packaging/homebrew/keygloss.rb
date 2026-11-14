# Homebrew cask for Keygloss. Copy it to Casks/keygloss.rb in a tap repository
# (github.com/<owner>/homebrew-tap), then: brew install --cask <owner>/tap/keygloss
# On each release, set `version` and `sha256` from the release job's summary ("Homebrew cask").
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
  depends_on macos: ">= :big_sur"

  app "Keygloss.app"

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
