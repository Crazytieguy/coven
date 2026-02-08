Issue: MCP tool names are displayed as raw identifiers (e.g. `mcp__plugin_llms-fetch-mcp_llms-fetch__fetch`) — should be shortened to a readable form like `llms-fetch: fetch`
Status: draft

## Approach

### The naming convention

Claude Code MCP tool names follow the pattern `mcp__<server-key>__<tool-name>`, where `__` (double underscore) is the separator. The server key often has a `plugin_` prefix for marketplace plugins.

Example: `mcp__plugin_llms-fetch-mcp_llms-fetch__fetch`
- Prefix: `mcp`
- Server key: `plugin_llms-fetch-mcp_llms-fetch`
- Tool name: `fetch`

### Display format

Shorten to `<server>: <tool>` where `<server>` is the server key with `plugin_` prefix stripped. For the example above: `llms-fetch-mcp_llms-fetch: fetch`.

This isn't as short as the `llms-fetch: fetch` in the issue title, but further shortening the server key would require heuristics that may not generalize across MCP server naming conventions. The `plugin_` stripping alone removes the most predictable noise.

### Changes

All changes in `src/display/renderer.rs`:

1. **Add `display_tool_name()` function**:
   ```rust
   fn display_tool_name(name: &str) -> String {
       let parts: Vec<&str> = name.splitn(3, "__").collect();
       if parts.len() == 3 && parts[0] == "mcp" {
           let server = parts[1].strip_prefix("plugin_").unwrap_or(parts[1]);
           let tool = parts[2];
           format!("{server}: {tool}")
       } else {
           name.to_string()
       }
   }
   ```

2. **Use `display_tool_name()` in the 4 label format sites**:
   - Line 305: `format!("  [{n}] ▶ {display_name}  {detail}")` (subagent tool call)
   - Line 311: `format!("[{n}] {display_name}")` (subagent stored message label)
   - Line 462: `format!("[{n}] ▶ {display_name}  {detail}")` (main tool call)
   - Line 468: `format!("[{n}] {display_name}")` (main stored message label)

3. **Keep `format_tool_detail()` matching on the raw name** — MCP tools already fall through to the `_` catch-all which shows the first string field value. No change needed there.

4. **Re-record the `mcp_tool` VCR test** and update its snapshot — the display should show the shortened name.

## Questions

### How aggressively should we shorten the server key?

The server key after stripping `plugin_` is still `llms-fetch-mcp_llms-fetch` — two related substrings joined by `_`. We could try to deduplicate (e.g., pick the shorter of the two `_`-separated parts), but this is fragile and may produce wrong results for other MCP servers.

Options:
A. **Just strip `plugin_` prefix** — safe, predictable, works for all MCP tools. Display: `llms-fetch-mcp_llms-fetch: fetch`
B. **Also try to deduplicate `_`-separated parts** — shorter but heuristic-based. Display: `llms-fetch: fetch`
C. **Use only the last `_`-separated segment** of the server key — concise but may lose context. Display: `llms-fetch: fetch`

Answer:

## Review

