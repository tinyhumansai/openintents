# OpenIntents plugin for Claude Code

Adds the hosted OpenIntents MCP server and the `openintents` skill to Claude
Code, so you can ask for purchases in plain language ("get me a flat white
from the Blue Bottle on 5th, pickup 8:45, max $10").

```text
/plugin marketplace add tinyhumansai/openintents
/plugin install openintents@openintents
```

The first time Claude Code uses the server it asks you to sign in. Run `/mcp`,
pick `openintents`, choose **Authenticate**, and approve access in the browser
window that opens. There is no key to create or paste: Claude Code registers
itself with the server, keeps the token it is given and refreshes it. To disconnect, clear the
server's authentication from `/mcp` or revoke the connection in your
OpenIntents dashboard.

## Use an API key instead

A key still works, for scripts and for clients that cannot open a browser. Make
one at <https://openintents.io/dashboard/api-keys> (start with a test key,
`oi_test_...`) and add a server of your own, with the key as the bearer token.
Remove the plugin's `openintents` server first, or give yours another name, so
the two do not clash:

```bash
claude mcp add --transport http openintents https://api.openintents.io/mcp \
  --header "Authorization: Bearer $OPENINTENTS_API_KEY"
```

For a client that only speaks stdio, the CLI runs a local bridge to the same
server (progress and questions for you included):

```bash
OPENINTENTS_API_KEY=oi_test_... npx -y @tinyhumansai/openintents mcp
```

See <https://openintents.io/docs/mcp>.
