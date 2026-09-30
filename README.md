# fcm-listener

Rust FCM listener.

## Setup

Copy env example:

```sh
cp .env.example .env
```

Required file:

```txt
.env
```

`fcm_state.json` is created on first registration, then reused.

Run:

```sh
cargo run --quiet
```

## Env

```env
APP_ID=
API_KEY=
GCM_SENDER_ID=
PROJECT_ID=
FCM_ANDROID_PACKAGE=
FCM_ANDROID_CERT=

# Optional. Empty = Discord forwarding off.
DISCORD_WEBHOOK_URL=

# Required.
WEBHOOK_URL=
WEBHOOK_TOKEN=

# Optional. Debug logs. 1 = on, 0/empty = off.
FCM_DEBUG=0

# Optional. MCS health checks.
FCM_HEARTBEAT_INTERVAL_SECS=120
FCM_HEARTBEAT_ACK_TIMEOUT_SECS=30
```
