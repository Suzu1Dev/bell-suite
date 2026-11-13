class BellSuite < Formula
  desc "Unified local-first workspace orchestrator and contract for the 5 BELL Suite apps"
  homepage "https://github.com/Suzu1Dev/bell-suite"
  url "https://github.com/Suzu1Dev/bell-suite/archive/refs/heads/main.tar.gz"
  version "1.0.1"
  license "Apache-2.0"
  head "https://github.com/Suzu1Dev/bell-suite.git", branch: "main"

  depends_on "node"
  depends_on "git"

  def install
    system "npm", "install", *std_npm_args
    bin.install_symlink libexec.glob("bin/*")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/bell-suite --version")
    assert_match "doctor", shell_output("#{bin}/bell-suite --help")
  end
end
