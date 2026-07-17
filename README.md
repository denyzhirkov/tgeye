# tgeye

Local MCP server for Telegram. A regular Telegram bot (created via BotFather) collects
messages from the chats it is added to into a local SQLite database; AI agents read
that history through read-only [MCP](https://modelcontextprotocol.io/) tools.

- **Local-first** — messages never leave your machine; no cloud, no built-in LLM.
- **Bot API, not your account** — the bot only sees chats you add it to.
- **Read-only by default** — agents can read, search and paginate; nothing else.
- **Allowlist by default** — content is stored only for chats you explicitly allow.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/denyzhirkov/tgeye/main/install.sh | sh
```

Or grab a binary from [Releases](https://github.com/denyzhirkov/tgeye/releases).

## Quick start

1. Create a bot with [@BotFather](https://t.me/BotFather), copy the token.
   Disable *Privacy Mode* (`/setprivacy` → Disable) so the bot sees all group messages.
2. Add the bot to the Telegram group you want to collect.
3. In your project directory:

```sh
tgeye init                    # creates ./.tgeye (config, secrets 0600, database), asks for the token
tgeye run                     # start the collector; send a message to the group
tgeye chats list              # find the chat id
tgeye chats allow <chat-id>   # allow storing its content
tgeye doctor                  # verify everything is green
```

4. Connect to an MCP client, e.g. Claude Code:

```sh
claude mcp add tgeye -- tgeye run-mcp
```

Keep `tgeye run` running (a separate terminal or a background service) — it collects;
`run-mcp` serves the collected history to the agent over stdio.

## MCP tools

| Tool | Purpose |
|---|---|
| `telegram_get_server_info` | Bot identity, storage stats, limits |
| `telegram_health_check` | Database, migration and ingestion health |
| `telegram_list_chats` | Known chats with access status and message counts |
| `telegram_get_chat` | One chat's details and message statistics |
| `telegram_get_messages` | Messages of an allowed chat in a time range, cursor pagination |
| `telegram_get_recent_messages` | Latest messages, newest first |
| `telegram_get_message` | A single message by id |
| `telegram_get_message_context` | A message plus surrounding messages and reply ancestors |
| `telegram_get_reply_chain` | The chain of messages a message replies to |
| `telegram_search_messages` | Full-text search across allowed chats (ranked, highlighted) |
| `telegram_list_attachments` | Attachment metadata for a chat (filters: time, kind, downloaded) |
| `telegram_get_attachment_metadata` | One attachment's metadata and download state |
| `telegram_download_attachment` | Download an attachment to local storage (sha256, dedup, size-limited) |
| `telegram_get_media_group` | All attachments of one album |

Telegram content returned by the tools is untrusted user-generated data — agents are
warned to treat it as data, not instructions.

## Configuration

Everything lives in the project-local `./.tgeye/` directory (override with `--data-dir`
or `TGEYE_HOME`): `config.toml` (documented inline), `secrets.toml` (bot token,
owner-only permissions; `TGEYE_TELEGRAM_BOT_TOKEN` env takes priority), and the
SQLite database.

## License

MIT
