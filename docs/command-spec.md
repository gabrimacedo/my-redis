# Command Specification

Each command lists: syntax, what it does, the response, and error cases.
Commands are case-insensitive (SET, set, Set all work). Arguments shown
in CAPS are literals; lowercase are values.

---

## Phase 1: Connection

### PING

```
PING [message]
```

- No argument: responds `+PONG\r\n` (Simple String)
- With argument: responds with Bulk String of that argument

```
> PING
+PONG\r\n

> PING "hello"
$5\r\nhello\r\n
```

### ECHO

```
ECHO message
```

- Returns the message as a Bulk String.
- Error if no argument.

```
> ECHO "hello world"
$11\r\nhello world\r\n

> ECHO
-ERR wrong number of arguments for 'echo' command\r\n
```

---

## Phase 2: Core Key-Value

### SET

```
SET key value [EX seconds | PX milliseconds]
```

- Stores `value` at `key`. Overwrites any existing value and type.
- If key had a TTL (from a previous SET EX), the old TTL is discarded.
  The new SET only has a TTL if EX/PX is specified again.
- `EX seconds` — set expiry in seconds (positive integer).
- `PX milliseconds` — set expiry in milliseconds (positive integer).
- EX and PX are mutually exclusive. If both are provided, respond
  with an error.
- Responds `+OK\r\n`

```
> SET mykey "hello"
+OK\r\n

> SET mykey "hello" EX 10
+OK\r\n

> SET mykey "hello" EX 10 PX 10000
-ERR XX and NX options at the same time are not compatible\r\n
```

Note: real Redis supports NX, XX, GET, EXAT, PXAT, KEEPTTL options too.
Out of scope — implement only EX and PX.

### GET

```
GET key
```

- Returns the value as a Bulk String.
- If key does not exist: `$-1\r\n` (Null).
- If key exists but holds a non-string type (e.g., a list): error. -CHECK

```
> GET mykey
$5\r\nhello\r\n

> GET nonexistent
$-1\r\n

> GET mylist    (where mylist is a list)
-WRONGTYPE Operation against a key holding the wrong kind of value\r\n
```

---

## Phase 3: Key Management

### DEL

```
DEL key [key ...]
```

- Deletes one or more keys. Keys that don't exist are ignored.
- Returns Integer: the number of keys that were actually deleted.
- Works on keys of any type (strings, lists, etc.).

```
> DEL key1 key2 key3
:2\r\n           (if key1 and key3 existed, key2 didn't)
```

### EXISTS

```
EXISTS key [key ...]
```

- Returns Integer: the count of keys that exist.
- If the same key is listed twice and it exists, it counts twice.

```
> EXISTS key1 key2
:2\r\n

> EXISTS key1 key1
:2\r\n           (key1 counted twice)

> EXISTS nonexistent
:0\r\n
```

### TTL

```
TTL key
```

- Returns Integer: remaining time to live in seconds.
- Key exists but has no expiry: `:-1\r\n`
- Key does not exist: `:-2\r\n`

```
> SET mykey "hello" EX 10
+OK\r\n
> TTL mykey
:9\r\n

> SET permanent "data"
+OK\r\n
> TTL permanent
:-1\r\n

> TTL nonexistent
:-2\r\n
```

### Key Expiration Behavior

When a key's TTL reaches zero, it must become invisible:
- GET returns Null
- EXISTS returns 0
- TTL returns -2
- DEL on an expired key is a no-op (returns 0)

Two cleanup strategies (implement both):

**Lazy expiration**: on every read/write access to a key, check if it's
expired. If so, delete it and proceed as if it didn't exist.

**Active expiration**: a background sweep that periodically removes
expired keys. This prevents memory leaks from keys that are set with
a TTL and never accessed again. Run this every ~100ms or so — pick a
reasonable interval. Each sweep doesn't need to scan every key; sampling
a batch (e.g., 20 random keys) is sufficient and is what Redis does.

---

## Phase 4: Lists

Redis lists are ordered sequences of byte strings, implemented as
doubly-linked lists (fast push/pop at both ends, slow random access).
For your implementation, a `VecDeque<Vec<u8>>` is the right choice.

A list key and a string key are different types. If a key holds a
string, list commands on it return WRONGTYPE. If a key holds a list,
GET returns WRONGTYPE.

Lists are created implicitly by push commands and removed when they
become empty (the key disappears).

### LPUSH

```
LPUSH key value [value ...]
```

- Inserts values at the head (left) of the list.
- If key doesn't exist, creates a new list.
- If key holds a non-list type: WRONGTYPE error.
- Multiple values are inserted left to right, so
  `LPUSH mylist a b c` results in `[c, b, a]`.
- Returns Integer: the length of the list after the push.

```
> LPUSH mylist "world"
:1\r\n
> LPUSH mylist "hello"
:2\r\n
```

### RPUSH

```
RPUSH key value [value ...]
```

- Same as LPUSH but inserts at the tail (right).
- `RPUSH mylist a b c` results in `[a, b, c]`.
- Returns Integer: the length of the list after the push.

### LPOP

```
LPOP key
```

- Removes and returns the first element.
- If key doesn't exist: Null.
- If key holds a non-list type: WRONGTYPE error.
- If the list becomes empty after the pop, the key is deleted.

```
> LPOP mylist
$5\r\nhello\r\n

> LPOP nonexistent
$-1\r\n
```

### RPOP

```
RPOP key
```

- Same as LPOP but removes from the tail.

### LRANGE

```
LRANGE key start stop
```

- Returns elements from index `start` to `stop` (inclusive, 0-based).
- Negative indices count from the end: -1 is the last element,
  -2 is second to last, etc.
- Out-of-range indices are clamped (not an error).
  - `start` beyond the end of the list: returns empty array.
  - `stop` beyond the end: treated as the last element.
  - `start > stop` (after resolving negatives): returns empty array.
- Returns Array of Bulk Strings.
- If key doesn't exist: empty array `*0\r\n`.
- If key holds a non-list type: WRONGTYPE error.

```
> RPUSH mylist "a" "b" "c" "d"
:4\r\n
> LRANGE mylist 0 -1       (all elements)
*4\r\n$1\r\na\r\n$1\r\nb\r\n$1\r\nc\r\n$1\r\nd\r\n

> LRANGE mylist 1 2
*2\r\n$1\r\nb\r\n$1\r\nc\r\n

> LRANGE mylist 5 10        (start beyond list)
*0\r\n
```

### LLEN

```
LLEN key
```

- Returns Integer: the length of the list.
- If key doesn't exist: `:0\r\n`.
- If key holds a non-list type: WRONGTYPE error.

---

## Phase 5: Pub/Sub

Pub/Sub breaks the normal request/response pattern. Once a client
subscribes, it enters "subscriber mode" and can ONLY send SUBSCRIBE,
UNSUBSCRIBE, and PING. All other commands return an error.

Messages are fire-and-forget: if no one is subscribed to a channel when
a message is published, the message is lost.

### SUBSCRIBE

```
SUBSCRIBE channel [channel ...]
```

- Subscribes the client to one or more channels.
- For EACH channel subscribed, the server sends a 3-element array:

```
*3\r\n
$9\r\nsubscribe\r\n
$<len>\r\n<channel>\r\n
:<count>\r\n
```

Where `<count>` is the total number of channels this client is now
subscribed to. So `SUBSCRIBE ch1 ch2` produces TWO responses:

```
*3\r\n$9\r\nsubscribe\r\n$3\r\nch1\r\n:1\r\n
*3\r\n$9\r\nsubscribe\r\n$3\r\nch2\r\n:2\r\n
```

### UNSUBSCRIBE

```
UNSUBSCRIBE [channel ...]
```

- No arguments: unsubscribe from all channels.
- With arguments: unsubscribe from those channels.
- For each channel, sends a response like SUBSCRIBE but with
  "unsubscribe" and the decreasing count.
- When count reaches 0, the client exits subscriber mode and can
  send normal commands again.

### PUBLISH

```
PUBLISH channel message
```

- Sends `message` to all clients subscribed to `channel`.
- Returns Integer: the number of clients that received the message.
- This command is sent by a NON-subscribed client (the publisher).

When a subscribed client receives a published message, the server sends:

```
*3\r\n
$7\r\nmessage\r\n
$<len>\r\n<channel>\r\n
$<len>\r\n<message>\r\n
```

### Pub/Sub Architecture Notes

This is where your channel-based architecture gets interesting. A
subscribed client needs to receive messages that it didn't request —
they arrive asynchronously. This means the connection task for a
subscribed client needs to select between:
- Incoming data from the TCP socket (the client sending more commands)
- Incoming messages from a broadcast channel (other clients publishing)

Tokio's `broadcast` channel is a good fit for pub/sub channels: one
sender, multiple receivers, each receiver gets every message.

---

## Phase 6: Transactions

Transactions let a client queue up multiple commands and execute them
atomically — no other client's commands interleave.

### MULTI

```
MULTI
```

- Starts a transaction. Responds `+OK\r\n`.
- After MULTI, all commands (except EXEC, DISCARD, MULTI) are queued
  instead of executed. Each queued command responds `+QUEUED\r\n`.
- Calling MULTI inside a MULTI is an error:
  `-ERR MULTI calls can not be nested\r\n`

### EXEC

```
EXEC
```

- Executes all queued commands atomically.
- Returns Array: one element per queued command, in order, each being
  that command's response.
- If not in a transaction: `-ERR EXEC without MULTI\r\n`

```
> MULTI
+OK\r\n
> SET a 1
+QUEUED\r\n
> SET b 2
+QUEUED\r\n
> GET a
+QUEUED\r\n
> EXEC
*3\r\n+OK\r\n+OK\r\n$1\r\n1\r\n
```

The atomicity guarantee: while EXEC runs, no other client's commands
are processed. Since your store task processes commands sequentially
from a channel, this is natural — send the whole batch as a single
message and process it before reading the next message from the channel.

### DISCARD

```
DISCARD
```

- Aborts the transaction. Discards all queued commands.
- Responds `+OK\r\n`.
- If not in a transaction: `-ERR DISCARD without MULTI\r\n`

### Transaction Architecture Notes

Transaction state (are we in MULTI? what's queued?) is PER-CONNECTION,
not per-store. The connection task holds this state. When EXEC arrives,
the connection task bundles all queued commands and sends them as one
unit to the store task.

---

## Phase 7: Persistence (RDB Snapshot)

RDB persistence saves a point-in-time snapshot of the entire dataset to
disk. This is simpler than AOF (append-only file) and good for learning.

### How It Works

1. At a configured interval (e.g., every 60 seconds if at least 1 key
   changed), serialize the entire store to a file.
2. On server startup, if the file exists, load it to restore state.

### SAVE

```
SAVE
```

- Triggers a synchronous save. Blocks until complete.
- Responds `+OK\r\n` on success.

### File Format

Use whatever serialization format you want. Options:
- `bincode` or `serde` with a custom format (simplest)
- Roll your own binary format (more educational)
- Actual RDB format (complex, probably not worth it)

The pragmatic choice: use `serde` + a simple binary format. The
interesting engineering is in the snapshotting logic, not the file
format.

### Snapshotting Architecture Notes

The store task owns the data, so it handles serialization. A background
save means the store needs to either:
- Pause command processing briefly to serialize (simple, blocks clients)
- Clone the data and serialize the clone in a background task (non-blocking
  but uses more memory)

Start with the blocking approach. Optimize later if you want.

---

## Error Responses

All errors use the Error frame type (`-`). Common patterns:

```
-ERR wrong number of arguments for '<command>' command\r\n
-ERR unknown command '<command>'\r\n
-ERR value is not an integer or out of range\r\n
-WRONGTYPE Operation against a key holding the wrong kind of value\r\n
-ERR EXEC without MULTI\r\n
-ERR DISCARD without MULTI\r\n
-ERR MULTI calls can not be nested\r\n
-ERR <command> is not allowed in subscriber mode\r\n
```

Match these formats exactly — some Redis clients parse the error type
prefix (ERR, WRONGTYPE) to decide how to handle them.
