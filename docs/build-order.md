# Build Order

TDD: write the test first, watch it fail, then implement. Each step is
testable before moving to the next. Don't skip ahead.

## Step 1: RESP Codec

### 1a: Encoding
For each type (SimpleString, Error, Integer, BulkString, Null, Array):
- [x] Write test: assert Frame::encode produces the expected bytes
- [x] Implement encode for that type
- [x] Green

### 1b: Decoding
- [x] Write test: decode a valid Simple String from bytes, assert Frame + bytes consumed
- [x] Implement decode for Simple String
- [x] Write test: decode a valid Error
- [x] Implement
- [x] Write test: decode a valid Integer
- [x] Implement
- [x] Write test: decode a valid Bulk String (including empty: `$0\r\n\r\n`)
- [x] Implement
- [x] Write test: decode Null (`$-1\r\n`)
- [x] Implement
- [x] Write test: decode a valid Array (including empty and nested)
- [x] Implement

### 1c: Edge cases
- [ ] Write test: decode partial input (truncated mid-frame) returns Incomplete
- [ ] Implement Incomplete handling (should already work if parser checks buffer length)
- [ ] Write test: decode invalid prefix byte returns Error
- [ ] Write test: decode non-numeric length in Bulk String returns Error
- [ ] Write test: round-trip encode → decode for every type produces the original Frame

## Step 2: TCP Scaffolding + PING/ECHO

### 2a: Integration test harness
- [ ] Write test helper: spawn server on a random port, return address
- [ ] Write test helper: connect to server, send raw bytes, read response bytes

### 2b: PING
- [ ] Write test: send PING as RESP array, expect `+PONG\r\n`
- [ ] Write test: send `PING "hello"`, expect `$5\r\nhello\r\n`
- [ ] Build TCP listener, Connection type, store task, Command::Ping — until tests pass

### 2c: ECHO
- [ ] Write test: send `ECHO "hello"`, expect `$5\r\nhello\r\n`
- [ ] Write test: send `ECHO` with no args, expect ERR response
- [ ] Implement Command::Echo — until tests pass

### 2d: Unknown command
- [ ] Write test: send `FOOBAR`, expect `-ERR unknown command 'FOOBAR'\r\n`
- [ ] Implement unknown command handling

## Step 3: SET / GET

### 3a: SET
- [ ] Write test: send SET mykey hello, expect `+OK\r\n`
- [ ] Implement Command::Set, store logic

### 3b: GET
- [ ] Write test: SET then GET, expect the value back as Bulk String
- [ ] Write test: GET nonexistent key, expect Null (`$-1\r\n`)
- [ ] Implement Command::Get

### 3c: Overwrite
- [ ] Write test: SET key to "a", SET key to "b", GET returns "b"
- [ ] Should pass with existing implementation (if not, fix)

## Step 4: DEL / EXISTS / Expiration

### 4a: DEL
- [ ] Write test: SET two keys, DEL both, expect `:2\r\n`
- [ ] Write test: DEL nonexistent key, expect `:0\r\n`
- [ ] Implement Command::Del

### 4b: EXISTS
- [ ] Write test: SET a key, EXISTS returns `:1\r\n`
- [ ] Write test: EXISTS nonexistent returns `:0\r\n`
- [ ] Write test: EXISTS same key twice returns `:2\r\n`
- [ ] Implement Command::Exists

### 4c: SET with EX/PX
- [ ] Write test: SET key EX 1, GET immediately returns value
- [ ] Write test: SET key EX 1, sleep >1s, GET returns Null
- [ ] Write test: SET key PX 100, sleep >100ms, GET returns Null
- [ ] Write test: SET key EX 10 PX 10000, expect error
- [ ] Implement expiry parsing and storage

### 4d: TTL
- [ ] Write test: SET key EX 10, TTL returns positive integer
- [ ] Write test: SET key (no expiry), TTL returns `:-1\r\n`
- [ ] Write test: TTL nonexistent key returns `:-2\r\n`
- [ ] Implement Command::Ttl

### 4e: Lazy expiration
- [ ] Write test: SET key EX 1, sleep >1s, EXISTS returns `:0\r\n`
- [ ] Write test: SET key EX 1, sleep >1s, TTL returns `:-2\r\n`
- [ ] Implement lazy expiry check on every key access

### 4f: Active expiration
- [ ] Implement background sweep in store task loop
- [ ] Write test: SET key PX 50, sleep 200ms, check key is gone without accessing it
  (verify by checking internal state or by confirming GET returns Null — lazy would
  also catch this, so this test mainly validates the sweep runs)

## Step 5: Lists

### 5a: LPUSH / RPUSH
- [ ] Write test: LPUSH mylist "a", expect `:1\r\n`
- [ ] Write test: LPUSH mylist "b", expect `:2\r\n`
- [ ] Write test: RPUSH mylist "c", expect `:3\r\n`
- [ ] Write test: LPUSH on a string key, expect WRONGTYPE error
- [ ] Implement value enum (String vs List), LPUSH, RPUSH

### 5b: LPOP / RPOP
- [ ] Write test: RPUSH "a" "b" "c", LPOP returns "a"
- [ ] Write test: RPOP returns "c"
- [ ] Write test: pop until empty, key disappears (GET returns Null, not WRONGTYPE)
- [ ] Write test: LPOP nonexistent key returns Null
- [ ] Implement LPOP, RPOP

### 5c: LRANGE
- [ ] Write test: RPUSH "a" "b" "c" "d", LRANGE 0 -1 returns all four
- [ ] Write test: LRANGE 1 2 returns "b" "c"
- [ ] Write test: LRANGE 5 10 returns empty array
- [ ] Write test: LRANGE on nonexistent key returns empty array
- [ ] Write test: LRANGE on string key returns WRONGTYPE
- [ ] Implement LRANGE

### 5d: LLEN
- [ ] Write test: RPUSH 3 items, LLEN returns `:3\r\n`
- [ ] Write test: LLEN nonexistent returns `:0\r\n`
- [ ] Write test: LLEN on string key returns WRONGTYPE
- [ ] Implement LLEN

### 5e: WRONGTYPE for existing commands
- [ ] Write test: LPUSH key, then GET key returns WRONGTYPE
- [ ] Write test: SET key, then LPUSH key returns WRONGTYPE
- [ ] Verify / fix existing commands

## Step 6: Pub/Sub

### 6a: SUBSCRIBE + PUBLISH
- [ ] Write test: client A subscribes to "ch1", expect subscribe confirmation
- [ ] Write test: client B publishes "hello" to "ch1", expect client A receives message
- [ ] Write test: PUBLISH returns count of receivers
- [ ] Implement subscribe/publish flow

### 6b: UNSUBSCRIBE
- [ ] Write test: subscribe to "ch1" and "ch2", unsubscribe from "ch1",
  confirm count decreases, still receives on "ch2"
- [ ] Write test: unsubscribe from all (no args), count reaches 0
- [ ] Implement unsubscribe

### 6c: Subscriber mode restrictions
- [ ] Write test: subscribed client sends SET, expect error
- [ ] Write test: subscribed client sends PING, expect PONG (allowed)
- [ ] Implement subscriber mode check

## Step 7: Transactions

### 7a: MULTI / EXEC
- [ ] Write test: MULTI → SET a 1 → SET b 2 → GET a → EXEC, expect array of responses
- [ ] Write test: EXEC without MULTI, expect error
- [ ] Write test: nested MULTI, expect error
- [ ] Implement transaction queuing and execution

### 7b: DISCARD
- [ ] Write test: MULTI → SET a 1 → DISCARD → GET a, expect Null (nothing was set)
- [ ] Write test: DISCARD without MULTI, expect error
- [ ] Implement discard

### 7c: Atomicity
- [ ] Write test: two clients, both in MULTI, verify EXEC runs atomically
  (client A's batch completes fully before client B's)
- [ ] Should work naturally with channel-based store

## Step 8: Persistence

### 8a: SAVE + load
- [ ] Write test: SET data, send SAVE, restart server, GET data returns value
- [ ] Implement snapshot serialization, SAVE command, startup load

### 8b: Auto-save
- [ ] Write test: SET data, wait for auto-save interval, kill server,
  restart, data persists
- [ ] Implement periodic save in store task

## Step 9: CLI Client

### 9a: Basic REPL
- [ ] Write test: parse input "SET foo bar" into 3 words
- [ ] Write test: parse input `SET foo "hello world"` into 3 words (quoted string)
- [ ] Implement input parser

### 9b: Send / receive
- [ ] Manual test: start server, start client, run PING/SET/GET
- [ ] Implement REPL loop, frame encoding, TCP send/receive, response display
