class Shore < Formula
  desc "Terminal User Interface for chatting with multiple language models"
  homepage "https://github.com/MoonKraken/shore"
  url "https://github.com/MoonKraken/shore/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "" # This will be filled when you create a release
  license "MIT" # Update with your actual license
  head "https://github.com/MoonKraken/shore.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  def post_install
    # Shore creates ~/.shore/ automatically on first run
    # No additional setup needed
  end

  def caveats
    <<~EOS
      Shore stores databases in ~/.shore/
      
      To use Shore, you'll need to set API keys for the providers you want to use:
        export OPENAI_API_KEY="your-openai-api-key"
        export ANTHROPIC_API_KEY="your-anthropic-api-key"
        export GROQ_API_KEY="your-groq-api-key"
        export HF_API_KEY="your-huggingface-api-key"
    EOS
  end

  test do
    # Test that the binary exists and can show help
    assert_match "Shore", shell_output("#{bin}/shore --help")
  end
end

