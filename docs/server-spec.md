# Redis Server Spec

A Redis server is an in-memory data structure store that listens for client connections over TCP and processes commands encoded in the RESP protocol.

---

## Networking

- Listen on a TCP port (default `6379`, bind to `127.0.0.1`)
- Accept multiple concurrent client connections
- Each connection is independent — commands from one client do not block another
- A connection stays open until the client disconnects or the server shuts down
- Read incoming bytes into a buffer, parse complete RESP frames, process them, and write the response back

### Partial Reads

TCP is a stream protocol. A single `read()` call may return half a frame, one frame, or multiple frames. The server must buffer incoming bytes and only attempt to parse when enough data has arrived. If a frame is incomplete, keep the bytes and wait for more.

---

## RESP Protocol (v2)

The server must parse and produce RESP2 frames.

### Data Types

| Type          | First Byte | Terminator     | Description                        |
|---------------|------------|----------------|------------------------------------|
| Simple String | `+`        | `\r\n`         | Single-line status string          |
| Error         | `-`        | `\r\n`         | Single-line error message          |
| Integer       | `:`        | `\r\n`         | Signed 64-bit integer              |
| Bulk String   | `$`        | length-prefixed| Binary-safe string with byte count |
| Array         | `*`        | element count  | Ordered sequence of frames         |

### Null Encoding

Null is represented as a Bulk String with length `-1`: `$-1\r\n`. There is also Null Array (`*-1\r\n`), but Null Bulk String is far more common.

### Incoming Format

All client commands arrive as an Array of Bulk Strings. The first element is the command name (case-insensitive), the rest are arguments.

### Error Response Convention

Error messages follow the format: `ERR <message>` for general errors, or `WRONGTYPE <message>` for type mismatches. The error type prefix (before the space) is part of the convention but not strictly required.

---

## Data Store

The core of the server is a key-value store held entirely in memory.

### Key Space

- Keys are always strings (binary-safe)
- Each key maps to a value of a specific type
- A key can only hold one type at a time — setting a new value on an existing key replaces it entirely (and its type)

### Value Types

The server supports the following value types:

**String** — A binary-safe sequence of bytes. Can represent text, numbers, or raw binary. Commands like `INCR` interpret the bytes as a decimal integer and operate on them numerically, returning an error if the value is not a valid integer.

**List** — An ordered sequence of strings. Supports push/pop from both ends. Maintains insertion order.

### Type Enforcement

If a command expects a specific value type and the key holds a different type, the server must respond with a `WRONGTYPE` error. For example, running `LPUSH` on a key that holds a String is an error.

Exception: `DEL`, `EXISTS`, `TTL`, and `TYPE` work on any key regardless of its value type.

---

## Key Expiration

Keys can have a TTL (time to live). Once the TTL elapses, the key is considered expired and must behave as if it doesn't exist.

### Setting Expiration

- `SET key value EX seconds` — Set with TTL in seconds
- `SET key value PX milliseconds` — Set with TTL in milliseconds

### Expiration Behavior

- An expired key must not be returned by `GET` or any other read command
- `TTL` on an expired key returns `-2` (same as a nonexistent key)
- Setting a new value on a key (via `SET`) removes any existing TTL unless a new one is specified
- `DEL` on an expired key returns `0` (it's already gone)

### Expiration Cleanup

There are two strategies, and a real implementation typically uses both:

**Lazy expiration** — Check if a key is expired whenever it is accessed. If expired, delete it and act as if it doesn't exist. This is the minimum required.

**Active expiration** — Periodically scan a sample of keys with TTLs and delete any that have expired. This prevents memory from being held by keys that are never accessed again. This is an optimization and not required for correctness, but important for memory hygiene.

---

## Commands

### Connection

**PING**
- Arguments: none, or a single optional message
- With no arguments: respond with Simple String `PONG`
- With an argument: respond with Bulk String containing the argument
- Purpose: connection health check

**ECHO message**
- Arguments: exactly one
- Respond with Bulk String containing the message
- Purpose: testing

### String Commands

**SET key value [EX seconds | PX milliseconds]**
- Arguments: key, value, and optional expiration flags
- Store the value as a String under the given key
- Overwrites any existing value and type at that key
- If `EX` or `PX` is provided, set the TTL accordingly
- Respond with Simple String `OK`

**GET key**
- Arguments: exactly one
- If the key exists and holds a String: respond with Bulk String containing the value
- If the key does not exist: respond with Null
- If the key holds a non-String type: respond with `WRONGTYPE` error

**INCR key**
- Arguments: exactly one
- If the key does not exist: treat as if the value were `0`, set it to `1`, respond with Integer `1`
- If the key holds a String that is a valid 64-bit signed integer: increment by 1, store the new value, respond with the new Integer
- If the value is not a valid integer: respond with an error
- If the key holds a non-String type: respond with `WRONGTYPE` error

### Key Commands

**DEL key [key ...]**
- Arguments: one or more keys
- Delete each specified key (if it exists)
- Respond with Integer: the number of keys that were actually deleted
- Works on any value type

**EXISTS key [key ...]**
- Arguments: one or more keys
- Respond with Integer: the number of specified keys that exist
- If the same key is listed multiple times, it is counted multiple times

**TTL key**
- Arguments: exactly one
- If the key exists and has a TTL: respond with Integer (remaining seconds)
- If the key exists but has no TTL: respond with Integer `-1`
- If the key does not exist: respond with Integer `-2`

**TYPE key**
- Arguments: exactly one
- If the key exists: respond with Simple String naming the type (`string`, `list`)
- If the key does not exist: respond with Simple String `none`

### List Commands

**LPUSH key element [element ...]**
- Arguments: key followed by one or more elements
- If the key does not exist: create a new list
- Insert all elements at the head (left) of the list
- Multiple elements are inserted one by one from left to right, so `LPUSH mylist a b c` results in the list `[c, b, a]`
- Respond with Integer: the length of the list after the operation
- If the key holds a non-List type: respond with `WRONGTYPE` error

**RPUSH key element [element ...]**
- Arguments: key followed by one or more elements
- Same as `LPUSH`, but inserts at the tail (right) of the list
- `RPUSH mylist a b c` results in `[a, b, c]`
- Respond with Integer: the length of the list after the operation

**LPOP key**
- Arguments: exactly one
- Remove and return the first (leftmost) element of the list
- If the key does not exist: respond with Null
- If the list becomes empty after the pop: delete the key entirely
- Respond with Bulk String containing the removed element

**RPOP key**
- Arguments: exactly one
- Same as `LPOP` but removes from the tail (right)

**LRANGE key start stop**
- Arguments: key, start index, stop index
- Return a contiguous range of elements from the list
- Indices are zero-based. Negative indices count from the end (`-1` is the last element, `-2` second to last)
- `start` and `stop` are both inclusive
- If `start` is beyond the end of the list: respond with an empty Array
- If `stop` is beyond the end of the list: treat it as the last element
- If the key does not exist: respond with an empty Array
- Respond with Array of Bulk Strings

**LLEN key**
- Arguments: exactly one
- If the key exists and holds a List: respond with Integer (the length)
- If the key does not exist: respond with Integer `0`
- If the key holds a non-List type: respond with `WRONGTYPE` error

### Pub/Sub

Pub/Sub is a messaging pattern where publishers send messages to named channels and subscribers receive them. Messages are fire-and-forget — if no one is subscribed, the message is lost.

**SUBSCRIBE channel [channel ...]**
- Arguments: one or more channel names
- The client enters Pub/Sub mode
- For each channel subscribed, the server sends an Array of three elements: the string `subscribe`, the channel name, and an Integer representing the total number of channels this client is now subscribed to
- While in Pub/Sub mode, the client can only send `SUBSCRIBE`, `UNSUBSCRIBE`, and `PING`. All other commands are rejected with an error.

**UNSUBSCRIBE [channel ...]**
- Arguments: zero or more channel names
- With arguments: unsubscribe from the listed channels
- With no arguments: unsubscribe from all channels
- For each channel unsubscribed, the server sends an Array of three elements: the string `unsubscribe`, the channel name, and the remaining subscription count
- When the subscription count reaches `0`, the client exits Pub/Sub mode and can send normal commands again

**PUBLISH channel message**
- Arguments: exactly two (channel name and message)
- Send the message to all clients subscribed to that channel
- Each subscribed client receives an Array of three elements: the string `message`, the channel name, and the message content
- Respond with Integer: the number of clients that received the message
- This command is sent by a regular client, not one in Pub/Sub mode

### Transactions

Transactions allow a client to execute a group of commands atomically — no other client's commands will be interleaved.

**MULTI**
- Arguments: none
- Enter transaction mode for this connection
- Respond with Simple String `OK`
- After `MULTI`, all subsequent commands (except `EXEC`, `DISCARD`, and some exceptions) are not executed — instead they are queued, and the server responds with Simple String `QUEUED` for each

**EXEC**
- Arguments: none
- Execute all queued commands in order
- Respond with an Array containing the result of each command, in order
- After `EXEC`, the client exits transaction mode
- If `MULTI` was not called first: respond with an error
- If any command was malformed (wrong arg count, etc.) during queueing, the entire transaction is aborted on `EXEC` and an error is returned

**DISCARD**
- Arguments: none
- Discard all queued commands and exit transaction mode
- Respond with Simple String `OK`
- If `MULTI` was not called first: respond with an error

---

## Persistence

Persistence allows the data to survive server restarts. There are two standard approaches.

### RDB (Snapshotting)

- Periodically serialize the entire data store to a binary file on disk
- On startup, if the file exists, load it to restore state
- Simple to implement but data written between snapshots is lost on crash

### AOF (Append-Only File)

- Log every write command to a file as it is executed
- On startup, replay the file to rebuild state
- More durable than RDB — at most one command is lost on crash
- The file grows over time and may need compaction (rewriting it with just the minimal set of commands that produces the current state)

Either approach (or both) is valid. AOF is simpler to start with conceptually since you're just writing RESP-encoded commands to a file.

---

## Concurrency Model

- The server must handle multiple clients concurrently
- Commands from different clients can be interleaved freely
- Each individual command is atomic — no partial execution visible to other clients
- The data store is shared across all connections
- Transaction guarantees (MULTI/EXEC) mean the queued commands execute as one uninterruptible batch

---

## Error Responses

The server must return appropriate errors for:

- **Wrong number of arguments** — `ERR wrong number of arguments for '<command>' command`
- **Wrong type** — `WRONGTYPE Operation against a key holding the wrong kind of value`
- **Not an integer** — `ERR value is not an integer or out of range`
- **Unknown command** — `ERR unknown command '<command>'`
- **Command used outside its mode** — e.g., `EXEC` without `MULTI`, or non-pub/sub commands during Pub/Sub mode
