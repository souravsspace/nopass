# Homebrew formula for nopass.
#
# This file lives in your tap repository (github.com/souravsspace/homebrew-tap)
# as Formula/nopass.rb. See RELEASING.md for the full publish flow.
class Nopass < Formula
  desc "Fast, self-contained password manager"
  homepage "https://github.com/souravsspace/nopass"
  url "https://github.com/souravsspace/nopass/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "51cd64f81e1187f5ef776bf5cc7aaa1841d63bd555fbd03b5c60b304d4f77790"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/nopass-cli")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nopass --version")
  end
end
