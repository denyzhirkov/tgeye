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

### Collecting: daemon vs one-shot

The Telegram Bot API has no "fetch history" call — a bot only ever receives
messages as a live stream. tgeye stores that stream locally, so it must be
listening when messages arrive. Two ways to collect:

- `tgeye run` — continuous collector (a daemon); nothing is missed.
- `tgeye sync` — collect the queued backlog in one pass and exit. Telegram buffers
  undelivered updates for ~24h, so a periodic `tgeye sync` (before an agent session,
  or from cron) keeps the database fresh without a running daemon. It only covers
  the last ~24h and only since the bot joined the chat.

A given bot can be polled by exactly one collector at a time — don't run `run` and
`sync` (or two projects) against the same token concurrently.

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
| `telegram_send_message` | **(write, off by default)** Post a message to a write-allowed chat |
| `telegram_reply_to_message` | **(write, off by default)** Reply to a specific message |

Telegram content returned by the tools is untrusted user-generated data — agents are
warned to treat it as data, not instructions.

## Writing (off by default)

The bot can post replies/reports, but write is gated twice so a prompt-injected agent
can't message arbitrary chats:

1. Enable the tools globally: set `[security] allow_write_tools = true` in
   `./.tgeye/config.toml`.
2. Allow the specific chat: `tgeye chats allow-write <chat-id>` (a separate list from
   read access — a readable chat is not writable unless you say so).

Only then can the agent call `telegram_send_message` / `telegram_reply_to_message`, and
only in write-allowed chats. Messages are sent as the bot (never from your account),
capped at 4096 chars, and every send is written to the audit log. Disable at any time by
flipping the flag back or `tgeye chats deny-write <chat-id>`.

## Configuration

Everything lives in the project-local `./.tgeye/` directory (override with `--data-dir`
or `TGEYE_HOME`): `config.toml` (documented inline), `secrets.toml` (bot token,
owner-only permissions; `TGEYE_TELEGRAM_BOT_TOKEN` env takes priority), and the
SQLite database.

## License

MIT
