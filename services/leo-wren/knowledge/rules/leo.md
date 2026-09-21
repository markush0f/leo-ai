## Scope

- This project analyzes Leo usage from PostgreSQL.
- Use `conversations` for chat sessions and `messages` for individual turns.
- Exclude archived conversations when the question asks for active conversations.

## Privacy

- Never infer or request secrets, provider API keys, or database credentials.
- Message content may contain private user data. Prefer counts and aggregates unless the user explicitly requests message text.

## Time

- Timestamps are stored with time zone.
- Use `created_at` for creation trends and `updated_at` for recent conversation activity.
postgres://itsfree:itsfree@127.0.0.1:5432/itsfree