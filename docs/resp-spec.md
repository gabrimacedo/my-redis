# RESP Protocol Specification

RESP (Redis Serialization Protocol) is the wire protocol used for all
client-server communication. Every message — request and response — is a
RESP value.

All communication is over TCP (default port 6379).

## Data Types

There are 6 types. Each starts with a one-byte prefix and ends with CRLF
(\r\n).

### Simple String — prefix `+`

```
+OK\r\n
```

- Everything between `+` and `\r\n` is the string.
- Cannot contain `\r` or `\n` inside the string.
- Used for short, non-binary server responses (e.g., OK, PONG).

### Error — prefix `-`

```
-ERR unknown command 'FOO'\r\n
-WRONGTYPE Operation against a key holding the wrong kind of value\r\n
```

- Same format as Simple String but indicates an error.
- Convention: first word is the error type (ERR, WRONGTYPE, etc.),
  rest is the message. Your implementation doesn't need to parse this —
  treat the whole thing as one string.

### Integer — prefix `:`

```
:42\r\n
:-1\r\n
:0\r\n
```

- A signed 64-bit integer.
- Used for command responses that return numbers (DEL returns count of
  deleted keys, EXISTS returns 0 or 1, etc.).

### Bulk String — prefix `$`

```
$5\r\nhello\r\n
$0\r\n\r\n
$-1\r\n
```

- Format: `$<length>\r\n<data>\r\n`
- `<length>` is the byte count of `<data>`.
- Binary-safe: the data can contain any bytes, including `\r\n`.
  Length tells you where it ends, not the CRLF.
- `$0\r\n\r\n` is an empty string (0 bytes, then CRLF terminator).
- `$-1\r\n` is Null (distinct from empty string). This is how Redis
  represents "key does not exist" in GET responses.

### Array — prefix `*`

```
*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nhello\r\n
*0\r\n
*-1\r\n
```

- Format: `*<count>\r\n<element_1><element_2>...<element_N>`
- `<count>` is the number of elements that follow.
- Elements can be ANY RESP type, including nested arrays.
- `*0\r\n` is an empty array.
- `*-1\r\n` is a Null array (used in some responses, treat same as Null
  for your purposes).

### Null — represented via Bulk String or Array

RESP2 has no dedicated Null type. Null is encoded as:
- `$-1\r\n` (null bulk string) — this is the common one
- `*-1\r\n` (null array) — rare, you'll encounter it with some commands

In your Frame enum, both map to `Frame::Null`.

## Client → Server

Clients ALWAYS send commands as Arrays of Bulk Strings:

```
*2\r\n$4\r\nPING\r\n$5\r\nhello\r\n
```

This means: array of 2 elements, first is "PING", second is "hello".

You will never receive Simple Strings, Integers, or Errors from a
client. Only Arrays of Bulk Strings.

## Server → Client

The server responds with any RESP type, depending on the command. Each
command's spec defines its response type.

## Inline Commands

Redis also supports "inline" commands — raw text without RESP encoding:

```
PING\r\n
SET mykey hello\r\n
```

These are space-separated words terminated by \r\n, with no RESP framing.
Useful for manual telnet testing. Optional to implement — redis-cli uses
RESP, not inline. Skip this unless you want to support raw telnet.

## Parsing Strategy

When parsing from a byte buffer:
1. Check if you have at least one byte (the type prefix).
2. Find the first `\r\n` to read the type-specific header.
3. For Bulk Strings: ensure you have `length + 2` more bytes after the
   header (data + trailing CRLF).
4. For Arrays: recursively parse `count` elements.
5. If at any point you don't have enough bytes, return "incomplete" —
   the caller should read more from the socket and try again.
6. On success, return the parsed Frame AND how many bytes were consumed,
   so the caller can advance the buffer.
