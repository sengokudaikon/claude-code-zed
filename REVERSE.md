# Network Traffic Analysis for Claude Code WebSocket Protocol

This document records methods to capture and analyze network traffic for understanding the Claude Code WebSocket protocol implementation.

## Process 

- first the vscode extension will create the `[port].lock` file in `~/.claude/ide/`, port number is randomly generated.

```json
{"pid":45272,"workspaceFolders":["/home/isomo/rust/claude-code-zed"],"ideName":"Visual Studio Code","transport":"ws","authToken":"9048d76f-acab-4fbc-87be-1c7ab575d94e"}
```

- then the claude code cli will check the lock file to try link the [port] on tcp, here we used the tcpdump tool to capture the traffic on port `59791`, then to start the claude code cli:

```bash
sudo tcpdump -i lo -A -tttt port 59791
2025-07-08 13:41:35.156734 IP localhost.42210 > localhost.59791: Flags [S], seq 4189769086, win 65495, options [mss 65495,sackOK,TS val 595966155 ecr 0,nop,wscale 7], length 0
E..<Z.@.@..................~.........0.........
#...........
2025-07-08 13:41:35.156744 IP localhost.59791 > localhost.42210: Flags [S.], seq 3069872564, ack 4189769087, win 65483, options [mss 65495,sackOK,TS val 595966155 ecr 595966155,nop,wscale 7], length 0
E..<..@.@.<..........................0.........
#...#.......
2025-07-08 13:41:35.156752 IP localhost.42210 > localhost.59791: Flags [.], ack 1, win 512, options [nop,nop,TS val 595966155 ecr 595966155], length 0
E..4Z.@.@............................(.....
#...#...
2025-07-08 13:41:35.188319 IP localhost.42210 > localhost.59791: Flags [P.], seq 1:357, ack 1, win 512, options [nop,nop,TS val 595966187 ecr 595966155], length 356
E...Z.@.@..W...............................
#...#...GET / HTTP/1.1
User-Agent: claude-code/1.0.44
X-Claude-Code-Ide-Authorization: 9048d76f-acab-4fbc-87be-1c7ab575d94e
Sec-WebSocket-Version: 13
Sec-WebSocket-Key: dJiOVWbgBNsSP3O+0l1mOw==
Connection: Upgrade
Upgrade: websocket
Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits
Sec-WebSocket-Protocol: mcp
Host: 127.0.0.1:59791


2025-07-08 13:58:25.784277 IP localhost.59791 > localhost.46700: Flags [.], ack 357, win 509, options [nop,nop,TS val 596976783 ecr 596976783], length 0
E..4..@.@..............l>..^\5[7.....(.....
#.$.#.$.
2025-07-08 13:58:25.784654 IP localhost.59791 > localhost.46700: Flags [P.], seq 1:159, ack 357, win 512, options [nop,nop,TS val 596976783 ecr 596976783], length 158
E.....@.@..:...........l>..^\5[7...........
#.$.#.$.HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: tQ2LNXBALh89K3IVhcqZ6FyInfY=
Sec-WebSocket-Protocol: mcp


2025-07-08 13:58:25.784660 IP localhost.46700 > localhost.59791: Flags [.], ack 159, win 511, options [nop,nop,TS val 596976783 ecr 596976783], length 0
E..4.B@.@.a..........l..\5[7>........(.....
#.$.#.$.
2025-07-08 13:58:25.788982 IP localhost.46700 > localhost.59791: Flags [P.], seq 357:536, ack 159, win 512, options [nop,nop,TS val 596976787 ecr 596976783], length 179
E....C@.@.`..........l..\5[7>..............
#.$.#.$........d{...t..."...n...a...e..Fp...m..^{...o...o...r...n..F2..Q-..I1..H"...a...i...s..."...t..^{..H"...e..-n..F:..
a..F:...a...-...e..Fv...i..F:..J0..P"..H"...n..."..V...H"..F:..
2025-07-08 13:58:25.789587 IP localhost.59791 > localhost.46700: Flags [P.], seq 159:341, ack 536, win 512, options [nop,nop,TS val 596976788 ecr 596976787], length 182
```

error traffic

```bash
2025-07-08T05:55:29.163716Z ERROR ThreadId(02) claude-code-server/src/websocket.rs:215: WebSocket error for 127.0.0.1:50614: WebSocket protocol error: Connection reset without closing handshake

2025-07-08 13:55:29.153943 IP localhost.cslistener > localhost.50614: Flags [P.], seq 1:130, ack 356, win 512, options [nop,nop,TS val 596800152 ecr 596800152], length 129
E....|@.@...........#(..UE.J...'...........
#.r.#.r.HTTP/1.1 101 Switching Protocols
connection: Upgrade
upgrade: websocket
sec-websocket-accept: 6LFdajTq+PSXJPZ62npP767jSrw=

**ISSUE IDENTIFIED**: Missing `Sec-WebSocket-Protocol: mcp` header in response. Client expects MCP protocol confirmation.

2025-07-08 13:55:29.153958 IP localhost.50614 > localhost.cslistener: Flags [.], ack 130, win 511, options [nop,nop,TS val 596800152 ecr 596800152], length 0
E..4..@.@.............#(...'UE.......(.....
#.r.#.r.
2025-07-08 13:55:29.163644 IP localhost.50614 > localhost.cslistener: Flags [F.], seq 356, ack 130, win 512, options [nop,nop,TS val 596800162 ecr 596800152], length 0
E..4..@.@.............#(...'UE.......(.....
#.r.#.r.
2025-07-08 13:55:29.163801 IP localhost.cslistener > localhost.50614: Flags [F.], seq 130, ack 357, win 512, options [nop,nop,TS val 596800162 ecr 596800162], length 0
E..4.}@.@..D........#(..UE.....(.....(.....
#.r.#.r.
2025-07-08 13:55:29.163829 IP localhost.50614 > localhost.cslistener: Flags [.], ack 131, win 512, options [nop,nop,TS val 596800162 ecr 596800162], length 0
E..4..@.@.............#(...(UE.......(.....
#.r.#.r.
```

## Solution Implemented

Fixed the WebSocket server in `claude-code-server/src/websocket.rs` to properly handle MCP protocol negotiation:

1. **Added proper imports**: `accept_hdr_async` and handshake types from `tokio_tungstenite`
2. **Replaced `accept_async`** with `accept_hdr_async` to handle custom header processing
3. **Added protocol negotiation**: Server now checks for `Sec-WebSocket-Protocol: mcp` in client request and responds with the same header
4. **Logging**: Added info log when MCP protocol is successfully negotiated

The server now responds with:
```
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: [hash]
Sec-WebSocket-Protocol: mcp
```

This prevents clients from closing the connection due to missing protocol confirmation.

## MCP Response Format Changes

### JavaScript Validation Functions
The client-side JavaScript validates specific response patterns:

```javascript
// TAB_CLOSED validation
function checkTabClosed(A) {
  return A.type === "result" && Array.isArray(A.data) && 
         A.data[0] === "object" && A.data[0] !== null && 
         "type" in A.data[0] && A.data[0].type === "text" && 
         "text" in A.data[0] && A.data[0].text === "TAB_CLOSED";
}

// FILE_SAVED validation (requires TWO text elements)
function checkFileSaved(A) {
  return A.type === "result" && Array.isArray(A.data) && 
         A.data[0]?.type === "text" && A.data[0].text === "FILE_SAVED" && 
         typeof A.data[1].text === "string";
}

// DIFF_REJECTED validation
function checkDiffRejected(A) {
  return A.type === "result" && Array.isArray(A.data) && 
         typeof A.data[0] === "object" && A.data[0] !== null && 
         "type" in A.data[0] && A.data[0].type === "text" && 
         "text" in A.data[0] && A.data[0].text === "DIFF_REJECTED";
}
```

### Server Response Format Updates
Updated MCP server responses to match expected validation patterns:

1. **TAB_CLOSED**: Single text element ✅
   ```rust
   vec![TextContent {
       type_: "text".to_string(),
       text: "TAB_CLOSED".to_string(),
   }]
   ```

2. **FILE_SAVED**: Two text elements (first="FILE_SAVED", second=additional info) ✅
   ```rust
   vec![
       TextContent {
           type_: "text".to_string(),
           text: "FILE_SAVED".to_string(),
       },
       TextContent {
           type_: "text".to_string(),
           text: format!("Diff accepted: {} -> {}", old_file_path, new_file_path),
       }
   ]
   ```

**Key Change**: FILE_SAVED responses now include two text elements instead of one, matching the client-side validation that expects `A.data[1].text` to be a string.


