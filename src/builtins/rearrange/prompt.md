# Squire rearrange format

`asq rearrange` moves, copies, deletes, and reorders 1-based line-range chunks
within a single file. Default mode is dry-run; pass `--yes` to write.

## Model

A spec has two parts: define chunks, then run exactly one action.
Ranges are 1-based inclusive: `10-20` includes both line 10 and line 20.
All ranges and anchors resolve against the original file snapshot.

## DSL

```ebnf
spec       = { line } ;
line       = blank | comment | file-decl | chunk-decl | action ;
comment    = "#" , { any } ;                  (* whole line only; inline # is literal *)
file-decl  = "file" , ws , path ;             (* exactly one per spec *)
chunk-decl = "chunk" , ws , name , "=" , range ;
action     = move | copy | delete | rearrange ;  (* exactly one per spec *)
move       = "move"   , ws , region , ws , "to" , ws , anchor ;
copy       = "copy"   , ws , region , ws , "to" , ws , anchor ;
delete     = "delete" , ws , region ;
rearrange  = "rearrange" , ws , namelist , "=>" , namelist , [ ws , "gap=" , gap ] ;
region     = range | name ;                   (* parsed as range iff first char is a digit *)
range      = number | number , "-" , number ; (* 1-based inclusive, start <= end *)
anchor     = "start" | "end" | "before" , ws , number | "after" , ws , number ;
namelist   = name , { "," , name } ;
gap        = "slot" | "drop" | "error" ;
name       = ( letter | "_" ) , { letter | digit | "_" } ;  (* identifier, not a keyword *)
number     = nonzero-digit , { digit } ;       (* >= 1 *)
```

Keywords reserved (cannot be chunk names): `file chunk move copy delete
rearrange to start end before after gap`.

## Gap (rearrange only)

Default `gap=slot`. Physical slots are the declared chunks ordered by line.
`rearrange` permutes slot contents while undeclared lines between slots (gaps)
stay pinned in their original inter-slot positions.

```text
file a.md
chunk A = 1-1
chunk B = 3-3
chunk C = 4-4
chunk D = 6-6
rearrange A, B, C, D => B, D, C, A
```

Given `A, h1, B, C, h2, D` this yields `B, h1, D, C, h2, A`.

`gap=drop` discards gaps; `gap=error` fails if any non-empty gap exists.
`from` is order-insensitive (a set); `to` defines the new order.

## CLI Usage (for AGENT)

```bash
# Pipe via stdin (recommended)
echo '<spec>' | asq rearrange --stdin --yes

# Dry-run first (default; no --yes needed)
asq rearrange --stdin < spec.txt

# From a file (use `asq tmp` for a scratch file)
asq tmp spec.txt          # prints a temp path
asq rearrange -f <path> --yes

# JSON output for machine parsing
asq rearrange --stdin --json < spec.txt
```

Flags: `-y/--yes` writes; `--dry-run` is the default; `--prompt` prints this.

## Safety

- Default is dry-run; nothing is written without `--yes`.
- Exactly one action per invocation; one target file (v1).
- Line numbers are 1-based inclusive.
- Same-file chunks must not overlap.
- Original newline style and encoding are preserved.
