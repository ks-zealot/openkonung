# Konung multiplayer client — DirectPlay network subsystem

Target: `CLIENT.EXE` (PE32/i386, base `0x400000`,
`sha256 adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac`).
All addresses are `CLIENT.EXE`-specific.

This document describes the client's network layer: DirectPlay setup, session
join, the receive queue and packet dispatch, the reliable send queue, the
timeout/`Server failed` gate, and the runtime log. It is static evidence plus
one local unattended run; no server was available, so no packet exchange was
observed.

## 1. Imports and GUIDs

| item | value | evidence |
|---|---|---|
| `DPLAYX.dll::DirectPlayEnumerateA` | thunk `0x43976A` | `ev_21ef6f44d333ad9b671efcb1ef50e8851d576e02e9ebb89fc11f9d12d5399a3d` |
| `DPLAYX.dll::DirectPlayLobbyCreateA` | thunk `0x439764` | same |
| `CLSID_DirectPlay` | `D1EB6D20-8923-11D0-9D97-00A0C90A43CB` @ `0x440B5C` | bytes read from artifact |
| DirectPlay interface IID | `133EFE41-32DC-11D0-9CFB-00A0C90A43CB` @ `0x440B4C` (DirectPlay 3A family) | same |
| application/lobby GUID | `36E95EE0-8577-11CF-960C-0080C7534E82` @ `0x440B7C` | same; copied to `0x7785FC` in WinMain |
| lobby IID | `0194C220-A303-11D0-9C4F-00A0C905425E` @ `0x440BCC` | same |

The binary imports only the enumerate and lobby-create entry points; every other
DirectPlay operation is a COM vtable call on the interface created by
`CoCreateInstance(CLSID_DirectPlay, …)`, so there are no further import names to
read.

## 2. Session setup — `FUN_0042D1EC` (`0x42D1EC`, 901 bytes)

`ev_b30081c467dc0dda2f149a2cb951eb23c0008284becb89b1cf1a81a4a7d91afa`

The function takes one string (a server/connection spec) and performs:

1. `DirectPlayEnumerateA(callback 0x42EAFC, hwnd)` — enumerate service
   providers. The callback sets `DAT_007786B6`/`DAT_007786B7` and stores the
   selected provider GUID at `0x77860C` (`FUN_0042D7EC`,
   `ev_749c943d15bba57e80b688639e6395f49a949fd534062bd0d5c98feb10b18d02`).
2. If no provider was found → return 0.
3. Copy the connection string into a 64-byte local buffer, stopping at the
   first character `<= 0x20` (space) — i.e. only the first token is used.
4. `DirectPlayLobbyCreateA(0, &lobby, 0, 0, 0)`.
5. Query the lobby interface for the lobby IID `0x440BCC`
   (`QueryInterface`-shaped call at slot `+0`).
6. `lobby->[+0x38](&connection, 2, &out, &size)` — connection/settings call
   (exact slot name unresolved; argument shape matches the lobby connection
   API).
7. Release the queried lobby view and the lobby object.
8. `CoInitialize(NULL)` and
   `CoCreateInstance(CLSID_DirectPlay, NULL, CLSCTX_INPROC_SERVER, IID, &dp)`
   — the interface pointer is stored at **`0x778630`**.
9. `dp->[+0x98](0x757180, 0)` — connection initialisation using the lobby
   connection blob at `0x757180`.
10. `dp->[+0x34](&session, 0, callback 0x42D7EC, hwnd, 1)` — session/enumeration
    or open call with the callback and the window handle.
11. If the callback ran (`DAT_007786B7 != 0`):
    - `dp->[+0x60](&name, 1)`;
    - `dp->[+0x18](&sessionName 0x6EB0EC, &DPNAME, 0, 0, 0, 0)` — create the
      local player. The `DPNAME` at `local_134` contains
      `lpName = 0x77857C` (the Windows user name / `"Unknown"`).
    - success → return 1; failure → `dp->[+0x10]` + `dp->[+8]` (close/release) and
      return 0.
12. Any failure releases the interface and returns 0.

The interface pointer is **`_DAT_00778630`** and the session name buffer is
**`_DAT_006EB0EC`**; both are consumed by the send path.

> The DirectPlay vtable slot names are **partially unresolved**. The interface
> IID identifies the DirectPlay 3A family, but the observed slot offsets
> (`+0x18`, `+0x34`, `+0x60`, `+0x68`, `+0x98`) do not all match the
> IDirectPlay3 documentation order used in this analysis. Only the effects and
> argument shapes above are claimed as observed.

## 3. Per-frame network pump — `FUN_0042D094` (`0x42D094`)

`FUN_0042D094` is the frame function called from `WM_USER 0x400`:

```text
_DAT_006EB7C8++                 // frame counter
_DAT_006EB358 = 0
FUN_0042855E()                  // input + state handler
for (i = 0; i < 15 && FUN_0042D574(&recv); i++)
    FUN_0042CA4F(&recv)         // dispatch all packets in the receive buffer
FUN_0042A00B()                  // reliable-queue retry / timeout
if (_DAT_006EB358 != 0)
    FUN_0042D620()              // flush outgoing buffer
```

### 3.1 Receive — `FUN_0042D574` (`0x42D574`)

`ev_5d818daf2729df154e64d47343c362309cc3d5e57f3c21d5cceb2af03d67857a`

```c
if (dp == NULL) return 0;
size = 0x200;
hr = dp->[+0x64](dp, &fromID, &toID, 1, &recv_buf[1], &size);
if (hr == DPERR_NOMESSAGES) return 0;
if (hr == 0) {
    if (fromID == 0) return 0;
    recv_buf[0] = size - 1;       // stored length
    return size - 1;
}
return -1;
```

The receive buffer is **`0x6EB154`**: `+0` = payload length, `+1..` = payload.
The 5-argument call shape (`from`, `to`, `flags`, `data`, `&size`) matches
`IDirectPlay::Receive`; the vtable slot is `+0x64`.

### 3.2 Packet framing and dispatch — `FUN_0042CA4F` (`0x42CA4F`)

`ev_ebba0c83c293beae721cacce85befad11d8d276deb51110a9c17359ffc5ef6e6`

The receive buffer at `0x6EB154` holds a **stream of concatenated packets**:

```text
+0x00  u32  total_bytes_remaining
+0x04  u8   type            (low 6 bits = handler index; high bit 0x80 = venue close)
+0x05  ...  payload
```

The dispatcher:

```text
p = buffer + 4;
while (buffer->len != 0) {
    if (*p & 0x80) {                 // venue/player disconnect notification
        id = *(u16*)(p+1);
        remove the matching 0x20B venue record from 0x5BAAE0 and compact;
    }
    consumed = packet_handlers[*p & 0x3F](p);
    buffer->len -= consumed;
    p += consumed;
}
```

The handler table is at **`0x44896C`**, 64 dword slots, but only types
`0x00..0x1D` (30 entries) contain code pointers; entries `0x1E..0x3F` are not
valid code. The client therefore defines **30 packet types**:

| type | handler | type | handler |
|---|---|---|---|
| `0x00` | `0x42A0B8` | `0x0F` | `0x42B27F` |
| `0x01` | `0x42A1B7` | `0x10` | `0x42B32E` |
| `0x02` | `0x42CA28` (no-op/stub) | `0x11` | `0x42B69C` |
| `0x03` | `0x42A261` | `0x12` | `0x42CA28` |
| `0x04` | `0x42A566` | `0x13` | `0x42B789` |
| `0x05` | `0x42A594` | `0x14` | `0x42CA28` |
| `0x06` | `0x42A5E9` | `0x15` | `0x42B7DC` |
| `0x07` | `0x42A65D` | `0x16` | `0x42B8F4` |
| `0x08` | `0x42A85D` | `0x17` | `0x42BA3A` |
| `0x09` | `0x42A8D3` | `0x18` | `0x42BD83` |
| `0x0A` | `0x42CA28` | `0x19` | `0x42C0C0` |
| `0x0B` | `0x42CA28` | `0x1A` | `0x42C268` |
| `0x0C` | `0x42ACAB` | `0x1B` | `0x42C511` |
| `0x0D` | `0x42AF19` | `0x1C` | `0x42CA28` |
| `0x0E` | `0x42B125` | `0x1D` | `0x42C5E9` |

Type `0x0C` is decoded in part: it copies a 32-byte player name from
`0x7570B0` into the per-player name table `0x5B2ED8 + id*0x20`, then reads a
16-bit slot index from `p+6` and stores a pointer to
`0x6CA8DC + index*0x100` at `0x6EB764` / `0x757780`. This is the
player-join/name packet.

Packet semantics for the remaining 29 types are **UNKNOWN** (handlers not yet
decompiled); the table is the authoritative map for a follow-up.

### 3.3 Reliable send queue — `FUN_004284A0` (`0x4284A0`)

`ev_9c19b77d099a1ca6996369b875672b61e9dc67985d624f059c81a463f1a7fd73`

```c
void queue_or_send(buf, size, queue_flag) {
    if (queue_flag) {
        e = &venue[0x5BAAE0 + _DAT_006EB7BC * 0x20B];
        _DAT_006EB7BC++;
        e->retry = 0;
        e->size  = size;
        e->frame = _DAT_006EB7C8;
        memcpy(e->payload, buf, size);
    }
    if (_DAT_006EB358 + size > 0x1FF) {
        flush();                 // FUN_0042D620
        _DAT_006EB358 = 0;
    }
    memcpy(0x6EB35D + _DAT_006EB358, buf, size);
    _DAT_006EB358 += size;
}
```

- Venue record: `0x5BAAE0`, stride **0x20B**, count `_DAT_006EB7BC`. Fields:
  `+0` retry counter, `+4` size, `+8` frame stamp, `+0x0C` payload. The count is
  not explicitly bounded in the writer; the record area is a fixed DGROUP
  array and overflow is a latent risk.
- Outgoing frame buffer: **`0x6EB35D`**, length `_DAT_006EB358`, flushed when
  adding would exceed `0x1FF`.

### 3.4 Flush — `FUN_0042D620` (`0x42D620`)

`ev_3b4c3e51b9493e42f5463d58beb87d2306353c82980ab3948ca2471a7573fafa`

```c
if (dp == NULL) return 0;
id = _DAT_006EB7CE;                       // local player id
hr = dp->[+0x68](dp, _DAT_006EB0EC, 1, 1, &id, _DAT_006EB358 + 1);
if (hr == 0) { _DAT_006EB358 = 0; return 1; }
return 0;
```

The `+0x68` slot is a send call with the session name and local player id; the
exact DirectPlay method name is unresolved.

### 3.5 Retry and “Server failed” — `FUN_0042A00B` (`0x42A00B`)

`ev_e7fad12955227117ea0af8c4f6d8fb076cdfb888f196223a887739366f56238f`

```c
for (i = 0; i < _DAT_006EB7BC; i++) {
    e = &venue[0x5BAAE0 + i*0x20B];
    if (_DAT_006EB7C8 - e->frame > 0xF) {     // 16 frames (~0.96 s)
        e->retry++;
        if (e->retry > 0xF) {                 // 16 retries (~15 s)
            FUN_004169DA("Server failed", 0x10);
            return 0;
        }
        e->frame = _DAT_006EB7C8;
        FUN_004284A0(e->payload, e->size, 0); // resend
    }
}
return 1;
```

This is the only “Server failed” producer. It is called every frame from
`FUN_0042D094` after the receive drain.

## 4. Logging and fatal path — `FUN_004169DA` (`0x4169DA`)

`ev_6497ca04e3ca130964cd10aa3cdd407cc6935614b0f06c7dddf456eb8112c788`

```text
open("<root>CLIENT.LOG", append)        // mode string at 0x4400F1
fprintf(log, "%s rc=%i\n", message, rc); // format at 0x4400FF
fclose(log)
_DAT_006EB780 = 1;                      // request quit
FUN_0042D064(); FUN_0042CE90(); FUN_0042D85C();
close OBJECTS/GAME archive handles (0x75789C/0x7578B8)
_DAT_006EB7C0 = rc;
strcpy(0x6EAFD0, message);
```

`CLIENT.LOG` is the client's only persistent diagnostic output. It is written
by every loader failure and by the network timeout.

## 5. Local player state packet — `FUN_0041701E` (`0x41701E`)

`ev_d76d940afee8943d8c15594adea8bf383e37d3630e4b0c5c14221eef91a0e270`

Builds a **0x73-byte** record at `0x6EB55C` and sends it with
`FUN_004284A0(record, 0x73, 1)` (queued reliable):

```text
+0x00  u8   0x4C
+0x01  u8   player slot (0)
+0x03  ...  32-byte player name from 0x7570B0
+0x23  ...  8 bytes from 0x7570D0
+0x2B  ...  32 bytes from 0x757038
...
+0x4B  ...
```

It also sets `_DAT_006EB774 = 0`, `_DAT_006EB7AC = 0xA`, `_DAT_006EB7CE = 0xFF`
and copies many hero fields (`0x75758E..0x7575BF`) into the packet. This is the
client's player-state broadcast (used when entering the session).

`FUN_0041701E` is called from the session screen (state 5); its packet is the
best starting point for a future server-compatible protocol implementation.

## 6. Runtime corroboration

- `CLIENT.LOG` was created on the first local run and contained
  `ME640R.RES not found rc=16` (not a network error; see
  `CLIENT_EXE_DATA_FORMATS.md`).
- With the interface archives present, the client reached the main menu and
  stayed in the message loop; no `Server failed` line was written because the
  session screen was not entered.
- `DPLAYX.dll` is present only as 32-bit `SysWOW64\dplayx.dll` on the host; it
  loaded successfully.

## 7. Residual unknowns

1. Exact DirectPlay vtable slot names (IID is the 3A family, but the observed
   offsets do not all match the documented IDirectPlay3 order).
2. Semantics of 29 of the 30 packet types (`0x00..0x1D`); only `0x0C` is
   partially decoded.
3. Why the dispatcher masks type with `0x3F` when only 30 handler slots are
   code pointers; types `0x1E..0x3F` would call non-code data and are assumed
   never to be sent.
4. The server side (there is no server executable in the install; `SERVERS.CFG`
   is only a list of connection strings).
5. The lobby connection blob at `0x757180` (built from the `SERVERS.CFG`
   string) is not decoded.
