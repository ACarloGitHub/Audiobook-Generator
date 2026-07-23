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

# Every tool is also available as:  abg-cli call <tool> '<json>'
abg-cli call configure '{"action":"set","engine":"VoxCPM2 Q8_0","language":"Italian"}'
abg-cli call book '{"action":"load","path":"D:\\\\Books\\\\mybook.epub"}'
abg-cli call generate '{"action":"start","chapters":["Chapter 1","Chapter 2"]}'
abg-cli call recover '{"action":"list","root_dir":"D:\\\\Audiobooks"}'`;

const WORKFLOW = `1. configure set        pick engine, voice, language, reference audio (+ its transcript)
2. book load            load an epub / txt / md / docx / json, list its chapters
3. generate start       convert the whole book or selected chapters
4. recover list/retry   if some chunks fail, re-synthesize only those
   recover merge        rebuild the chapter MP3 after a successful retry`;

export function renderAgents(): string {
  return `
    <div class="card">
      <h2>Drive Audiobook Generator from AI agents</h2>
      <p class="field-help">
        The <code>abg-cli</code> companion program lets you — or an AI agent — use the app
        without opening this window: configure an engine, load a book, generate the audio and
        repair failures. It reads the same models and settings as the app, and the GPU-only
        rule applies (no GPU, no synthesis).
      </p>
      <p class="field-help">
        It ships inside the installer, in the <code>resources/cli/</code> folder next to the app
        executable. Add that folder to your PATH, or call it with its full path.
      </p>
    </div>

    <div class="card">
      <h2>Command line usage</h2>
      <textarea class="text-input log-area" rows="12" readonly>${escapeHtml(CLI_EXAMPLES)}</textarea>
      <div class="btn-row">
        <button class="btn-secondary" id="agents-copy-cli">📋 Copy examples</button>
      </div>
      <p class="field-help">
        Engine ids, voice ids and defaults: run <code>abg-cli status</code> or
        <code>abg-cli call configure '{"action":"get_parameters","engine":"…"}'</code> (JSON output).
        Extra engine parameters go in <code>--param key=value</code> pairs (repeatable) or in the
        <code>params</code> object of <code>configure set</code>.
      </p>
    </div>

    <div class="card">
      <h2>Use it as an MCP server (LM Studio)</h2>
      <p class="field-help">
        <code>abg-cli --mcp</code> speaks the MCP protocol, so agents in LM Studio can call the
        tools below. Register it in <code>%USERPROFILE%\\.cache\\lm-studio\\mcp.json</code> like this:
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
      <h2>The tools agents can call</h2>
      <p class="field-help">
        • <code>get_status</code> — storage folder, GPU devices, installed engines and models.<br/>
        • <code>configure</code> — session settings that persist across calls:
        <code>list_engines</code>, <code>list_voices</code> (with each voice's native language),
        <code>get_parameters</code> (documented min/max/default per engine), and
        <code>set</code> (engine, voice, language, reference audio, reference transcript,
        parameters). When the engine needs the reference transcript and it is missing, the tool
        says so — the agent should ask you for it.<br/>
        • <code>synthesize</code> — text or text file → WAV, using the session as fallback.<br/>
        • <code>book</code> — <code>load</code> a document (epub, txt, md, docx, json) and
        <code>chapters</code> to list its chapters.<br/>
        • <code>generate</code> — <code>start</code> converts the whole book or the chapter titles
        you pass; <code>delete_intermediate_chunks</code> removes chunk folders only when nothing
        failed.<br/>
        • <code>recover</code> — <code>list</code> finds interrupted books,
        <code>retry</code> re-synthesizes the failed chunks (same engine/parameters recorded at
        generation time), <code>merge</code> rebuilds the chapter MP3.
      </p>
    </div>

    <div class="card">
      <h2>Typical agent workflow</h2>
      <textarea class="text-input log-area" rows="6" readonly>${escapeHtml(WORKFLOW)}</textarea>
      <div class="btn-row">
        <button class="btn-secondary" id="agents-copy-workflow">📋 Copy workflow</button>
      </div>
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
  document.getElementById("agents-copy-workflow")?.addEventListener("click", () => {
    void navigator.clipboard.writeText(WORKFLOW);
  });
}
