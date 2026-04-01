# Redis Client Spec

A Redis client is a program that connects to a Redis server over TCP and communicates using the RESP protocol.

---

## Connection

- Open a TCP connection to the server (default `127.0.0.1:6379`)
- The connection is persistent — open it once and send multiple commands over the same connection
- Either side can close the connection at any time

---

## Sending Commands

Every command is encoded as a RESP Array of Bulk Strings and written to the TCP stream. The command name is the first element, arguments follow.

`SET key value` becomes an array of 3 bulk strings: `"SET"`, `"key"`, `"value"`.

Command names are case-insensitive — the server treats `set`, `SET`, and `Set` the same.

---

## Receiving Responses

After sending a command, read one complete RESP frame from the stream. The response type depends on the command:

- **Simple String** — Status replies like `OK`
- **Error** — When something goes wrong (wrong number of args, wrong type, etc.)
- **Integer** — For commands like `INCR`, `DEL` (returns count of deleted keys), `EXISTS`
- **Bulk String** — For commands that return data, like `GET`
- **Null** — When a key doesn't exist (`GET` on missing key)
- **Array** — For commands that return multiple values, like `LRANGE` or `KEYS`

The client must be able to parse all of these.

---

## Pipelining

A client can send multiple commands without waiting for each response. The server processes them in order and sends responses in the same order. The client then reads the responses sequentially.

This is an optimization, not a requirement. A basic client can do simple request-response first and add pipelining later.

---

## Pub/Sub Mode

When a client sends `SUBSCRIBE`, it enters a special mode. It no longer sends normal commands — instead it sits and receives push messages from the server whenever someone publishes to the subscribed channel. The message format is a RESP Array with three elements: the string `"message"`, the channel name, and the payload.

To leave this mode, the client sends `UNSUBSCRIBE`.

---

## Transaction Mode

When a client sends `MULTI`, the server starts queuing commands instead of executing them. Each subsequent command gets a `QUEUED` response. When the client sends `EXEC`, the server executes all queued commands atomically and returns an Array of all the responses. `DISCARD` cancels the transaction.

---

## Error Handling

- If the TCP connection drops, the client should detect it and either reconnect or report the error
- RESP Error frames are not fatal — they're just the server telling you a specific command failed. The connection stays open.

---

## What a Client Does NOT Do

- It doesn't store data itself
- It doesn't interpret commands — it just serializes them to RESP, sends them, and deserializes the response
- It doesn't need to know what commands exist — it blindly encodes whatever the user provides
