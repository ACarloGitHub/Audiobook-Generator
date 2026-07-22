import { escapeHtml } from "./helpers";

const MCP_JSON = `{
  "mcpServers": {
    "audiobook-generator": {
      "command": "C:\\\\path\\\\to\\\\abg-cli.exe",
      "args": ["--mcp"]
    }
  }
}`;

const CLI_EXAMPLES = `# List engines, models and GPU status
abg-cli status

# Quick voice test (single line of text)
abg-cli synthesize --engine OuteTTS-1.0-0.6B --text "Hello world" --out hello.wav

# Clone a voice from a reference WAV, full parameters
abg-cli synthesize --engine "VoxCPM2 Q8_0" --text-file script.txt --ref my_voice.wav --language Italian --out D:\\Voices\\voiceattack.wav --param temperature=0.7`;

export function renderAgents(): string {
  return `
    <div class="card">
      <h2>Drive Audiobook Generator from AI agents</h2>
      <p class="field-help">
        The <code>abg-cli</code> companion program lets you — or an AI agent — use the TTS engines
        without opening this window: synthesize speech to WAV files, pick engine, voice, reference
        audio and parameters. It reads the same models and settings as the app.
      </p>
      <p class="field-help">
        It ships inside the installer, in the <code>resources/cli/</code> folder next to the app
        executable. Add that folder to your PATH, or call it with its full path.
      </p>
    </div>

    <div class="card">
      <h2>Command line usage</h2>
      <textarea class="text-input log-area" rows="10" readonly>${escapeHtml(CLI_EXAMPLES)}</textarea>
      <div class="btn-row">
        <button class="btn-secondary" id="agents-copy-cli">📋 Copy examples</button>
      </div>
      <p class="field-help">
        Engine ids, voice ids and defaults: run <code>abg-cli status</code> (JSON output).
        Extra engine parameters go in <code>--param key=value</code> pairs (repeatable).
      </p>
    </div>

    <div class="card">
      <h2>Use it as an MCP server (LM Studio)</h2>
      <p class="field-help">
        <code>abg-cli --mcp</code> speaks the MCP protocol, so agents in LM Studio can call
        <code>get_status</code> and <code>synthesize</code> as tools. Register it in
        <code>%USERPROFILE%\\.cache\\lm-studio\\mcp.json</code> like this:
      </p>
      <textarea class="text-input log-area" rows="9" readonly id="agents-mcp-json">${escapeHtml(MCP_JSON)}</textarea>
      <div class="btn-row">
        <button class="btn-secondary" id="agents-copy-mcp">📋 Copy JSON</button>
      </div>
      <p class="field-help">
        Replace <code>C:\\\\path\\\\to\\\\abg-cli.exe</code> with the real path (the
        <code>resources/cli/</code> folder of your installation).<br/>
        <strong>Gotcha:</strong> when pasting manually into an existing <code>mcp.json</code>, copy only
        the content between <code>"mcpServers": {</code> and the matching closing brace — LM Studio
        merges it into the existing top-level key. Then restart LM Studio and enable the server in
        the chat's tool picker.
      </p>
    </div>

    <div class="card">
      <h2>What agents can do today</h2>
      <p class="field-help">
        • <code>get_status</code> — storage folder, live GPU memory, installed engines and models.<br/>
        • <code>synthesize</code> — text or text file → WAV, with engine, voice, language,
        reference audio for cloning, chunk size and any engine parameter.<br/>
        Full-book generation from agents is planned next; today it is done from the Generate panel.
      </p>
    </div>
  `;
}

export function attachAgentsListeners(): void {
  document.getElementById("agents-copy-cli")?.addEventListener("click", () => {
    void navigator.clipboard.writeText(CLI_EXAMPLES);
  });
  document.getElementById("agents-copy-mcp")?.addEventListener("click", () => {
    void navigator.clipboard.writeText(MCP_JSON);
  });
}
