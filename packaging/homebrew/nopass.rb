# Homebrew formula for nopass.
#
# This file lives in your tap repository (github.com/souravsspace/homebrew-tap)
# as Formula/nopass.rb. See RELEASING.md for the full publish flow.
class Nopass < Formula
  desc "Fast, self-contained password manager"
  homepage "https://github.com/souravsspace/nopass"
  url "https://github.com/souravsspace/nopass/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "57f0b92859fc64a564cd0c3f45f7b7a363fe00591289693a6d0e9a980fdbe10d"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/nopass-cli")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nopass --version")
  end
end
