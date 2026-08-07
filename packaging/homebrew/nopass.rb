# Homebrew formula for nopass.
#
# This file lives in your tap repository (github.com/souravsspace/homebrew-tap)
# as Formula/nopass.rb. See RELEASING.md for the full publish flow.
class Nopass < Formula
  desc "Fast, self-contained password manager"
  homepage "https://github.com/souravsspace/nopass"
  url "https://github.com/souravsspace/nopass/archive/refs/tags/v0.2.1.tar.gz"
  sha256 "35ab5dfc18f22cc255a71b8d0f678d4d3034768df6e0f32ca25c2be63b7dd491"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/nopass-cli")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nopass --version")
  end
end
