class Dbscope < Formula
  desc "Read-only schema intelligence for SQL databases"
  homepage "https://github.com/jayvenn21/dbscope"
  version "0.2.0"
  license "MIT OR Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/jayvenn21/dbscope/releases/download/v#{version}/dbscope-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/jayvenn21/dbscope/releases/download/v#{version}/dbscope-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/jayvenn21/dbscope/releases/download/v#{version}/dbscope-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/jayvenn21/dbscope/releases/download/v#{version}/dbscope-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "dbscope"
  end

  test do
    assert_match "dbscope", shell_output("#{bin}/dbscope --version")
  end
end
