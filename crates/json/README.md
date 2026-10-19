# `cdda-json`

The port's JSON reader and writer, matching the reference loader's acceptance
rules exactly. The reference is Cataclysm-DDA at tag `0.I-1` (commit `7b2efa5`),
cited by file and line wherever a rule is encoded.

## What "the reference's reader" is

Game data is **not** parsed by the reference's hand-written `TextJsonIn`.
`json_loader.cpp` obtains every document through `flexbuffer_cache`, which hands
the text to the vendored flatbuffers parser configured with `strict_json = true`
and `use_flexbuffers = true` (`src/flexbuffer_cache.cpp:59-70`). `TextJsonIn` is
used afterwards only to re-read the source when a parse failed, to produce a
nicer message (`flexbuffer_cache.cpp:70-126`). So the acceptance rules — and the
value model the game sees — are flatbuffers' lexer, value parser and flexbuffers
storage. That is what this crate reproduces.

## Tests

```
cargo test -p cdda-json                       # unit-level rules, ~8 s
cargo fmt --all && cargo test --workspace    # the workspace gate
```

The two fixture walks read the pinned checkout in place, never a copy:

* `tests/data_files.rs` parses all 3013 files under `data/json` and all 6220
  files under `data` (107 MB) and requires zero failures.
* `tests/reference_parity.rs` compares this crate's accept/reject decision *and*
  a digest of the parsed structure with the reference's own parser for all 6220
  files (key order, integer-versus-float typing, decoded string bytes, NUL
  handling).
* `tests/writer_parity.rs` compares the writer's compact and pretty bytes with a
  verbatim copy of `JsonOut` for a sample of real files.

The last two need a probe built from the reference's vendored flatbuffers:

```
crates/json/tools/build_reference_probe.sh    # needs a C++17 compiler
```

They skip (loudly) when the probe is absent.

## Findings: where the reference is odd, or platform-dependent

Verified by running the reference's own parser/writer, not inferred:

| Finding | Consequence |
| --- | --- |
| Only the **first** top-level value is read and nothing after it is checked (`idl_parser.cpp:3077-3086`) | `{} {}`, `1 2`, `{"a":1}xyz` are all *accepted*; a strict reader rejects them |
| Object members are stored **sorted by `strcmp`**, not in document order (`flexbuffers.h:1127-1151`) | The reference does not preserve member order; the port sorts on parse and cites it |
| Duplicate keys are a hard error, for any nesting level (`idl_parser.cpp:3033-3034`) | Also for keys that agree up to an embedded NUL, because keys are C strings: `{"a\u0000b":1,"a\u0000c":2}` is rejected |
| `+`/`-` followed by a letter is a *string constant* (`idl_parser.cpp:505-514`) | `-inf`, `+nan`, `+PI` are accepted and come out as strings, while bare `nan`/`inf`/`Infinity`, `True`, `NULL` are rejected |
| Integers are converted with `StringToInt`, i.e. decimal `strtoll` with an `errno` check (`util.h:303-327`, `util.h:407-410`) | `0x10` is an integer token that converts to **0**, and `12345678901234567890123` silently becomes **0** rather than saturating or erroring |
| Floats go through locale-independent `strtod` in the C locale (`util.h:346-345`, enforced by `cata_error_locale_dependent.h`) | `1.5`, `.5`, `1.` and C99 hex floats (`0x1p3` = 8) are accepted; overflow is `inf`, underflow is `0`, both silently |
| Comment syntax is C++, not JSON (`idl_parser.cpp:481-502`) | `//` and `/* */` are allowed wherever whitespace is; `///` must start its own line or it is an error. No shipped data file actually uses them — the data writes notes as `"//"` members — so this rule is verified synthetically against the reference parser |
| Both `"` and `'` delimit strings (`idl_parser.cpp:365-366`) | Single-quoted keys and values are accepted; `\xNN` and `\'` are valid escapes |
| A NUL byte is the end of input (`idl_parser.cpp:344-347`) | An embedded NUL silently truncates the document. The `mmap` path in `flexbuffer_cache::parse` relies on this without a terminator, which is only safe because the page tail is zero-filled — a file whose size is a page multiple is undefined behaviour in the reference |
| A UTF-8 BOM is accepted once, at the very start (`idl_parser.cpp:319-329`) | A partial BOM is `invalid utf-8 byte order mark`; a BOM after whitespace is `illegal character: code: -17` |
| Strings are validated as UTF-8, but only when they held an escape or a non-printable byte (`idl_parser.cpp:474-477`) | Rejections match, since printable ASCII is always valid UTF-8; invalid bytes in strings are `illegal UTF-8 sequence`, and outside strings `illegal character: code: <signed byte>` |
| At most 64 nested values (`idl.h:41-42`, `idl_parser.cpp:163-186`) | The 65th value errors with `maximum parsing depth 65 reached`; an empty container at the bottom does not count |
| `JsonOut` prints floats with `std::fixed` and `showpoint` at precision 6 (`json.cpp:2226-2237`) | `1.5` is written `1.500000`, `1e300` is written with 307 digits and no exponent, and values with more than six decimals — or more than 53 bits — do not survive a round trip |
| The same stream prints non-finite values as `inf`, `-inf`, `nan` (`json.cpp:2354`, verified) | The writer can emit text its own reader rejects; NaN is spelled `nan` by C++ and `NaN` by Rust, so the port spells it out |
| `JsonOut::write` leaves `/` and bytes above ASCII unescaped and escapes other control bytes as `\u00XY` with an upper-case low nibble (`json.cpp:2363-2404`) | `\u001B`, not `\u001b`; DEL is written raw |

Two behaviours cannot be settled from the source alone: the spelling of a
*negative* NaN (`-nan` prints as `nan` on this platform, `libstdc++` may differ)
and anything downstream of `strtod`'s rounding for hex floats with long
mantissas, where this port's conversion is not guaranteed correctly rounded.
Neither occurs in the shipped data.

## Deliberately left out of this slice

* The `TextJsonIn`/`TextJsonObject`/`TextJsonArray` navigation layer from
  `json.h`/`json.cpp`; the loader does not use it, and it is a large API whose
  rules would need their own oracle.
* The flexbuffer binary format, its on-disk cache and the `mmap`/mtime staleness
  logic (`flexbuffer_cache.cpp:133-507`). This crate parses text into a value;
  it does not cache.
* `JsonOut`'s streaming extras that game code uses but data never needs:
  `tell`/`seek`, bitsets, enum/`string_id` helpers, and char overloads.
* The `tools/format` formatter's layout heuristics (a 120-column reflow and
  special cases for `rows`, `blueprint` and `picture`); the writer exposes
  `wrap` per container instead, as `JsonOut` itself does.
* Reading compressed (`.gz`) sources: `read_maybe_compressed_file` is outside the
  reader, so this crate takes bytes.
