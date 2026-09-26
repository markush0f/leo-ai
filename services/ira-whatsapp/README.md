# Ira WhatsApp

Baileys companion so you can text Leo. It does not call the model itself: turns go to `ira-server`, which keeps the WhatsApp thread in Postgres (`channel = whatsapp`).

Leo answers only in your self-chat («Mensajes a ti mismo») and, if set, numbers in `WHATSAPP_ALLOW_PHONES`. Groups, status, audio, and images are ignored. An empty allowlist does not open the rest of the inbox.

```sh
# terminal 1
cargo run -p ira-server

# terminal 2
cd services/ira-whatsapp
npm install
npm start
```

Scan the QR from WhatsApp → Linked devices. Session files live in `WHATSAPP_AUTH_DIR` (default `~/.config/ira-ai/whatsapp`, mode `0700`). To link a different number:

```sh
npm run pair
```

```sh
IRA_API_URL=http://127.0.0.1:8787
WHATSAPP_ALLOW_PHONES=34600000000
WHATSAPP_AUTH_DIR=~/.config/ira-ai/whatsapp
```

`/help`, `/status`, and `/clear` are local. Other text is a normal Ira turn, tools included.

Baileys is not the official WhatsApp API. Linking a number can get that account banned. Do not commit the auth directory.
