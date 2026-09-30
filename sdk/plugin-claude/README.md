# OpenIntents plugin for Claude Code

Adds the hosted OpenIntents MCP server and the `openintents` skill to Claude
Code, so you can ask for purchases in plain language ("get me a flat white
from the Blue Bottle on 5th, pickup 8:45, max $10").

```text
/plugin marketplace add tinyhumansai/openintents
/plugin install openintents@openintents
```

The first tool call signs you in through the browser. Prefer a one-off
server without the plugin?

```bash
claude mcp add --transport http openintents https://api.openintents.io/mcp
```

See <https://openintents.io/docs/mcp>.
