#!/usr/bin/env python3
"""Generate `corpus/options/table.json` from the pinned CDDA reference checkout.

The reference builds its option table at startup by calling
`options_manager::add( ... )` once per option (`src/options.cpp`), plus one
`add_external` per `EXTERNAL_OPTION` entry in `data/core/external_options.json`
(`src/worldfactory.cpp:2060-2095`).  Nothing in the table is typed by hand: this
script parses those two sources, applies the same argument validation the
reference applies at registration time, and writes the result.

Every registration that the script cannot resolve statically is either
**reported as a finding** in `meta` or a **hard error**: the script never skips
a registration it did not understand.  The only escape hatch is `RUNTIME_SLOTS`
below, which lists, one by one, the slots whose value the reference computes at
runtime from the filesystem or the OS locale; their source text is recorded
verbatim so a change in the reference breaks the script instead of the table.

Usage:
    python3 tools/gen_options.py [--ref PATH] [--out PATH]
    python3 tools/gen_options.py --check        # fail if the output is stale

The output is reproducible: two runs on the same checkout produce identical
bytes (no timestamps, no directory listings, options sorted by name).
"""

import argparse
import json
import pathlib
import re
import struct
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_REF = REPO_ROOT / "references/Cataclysm-DDA"
DEFAULT_OUT = REPO_ROOT / "corpus/options/table.json"

# --------------------------------------------------------------------------
# Build profile.
#
# `options.cpp` guards some registrations with the preprocessor, and one
# registration (`FULLSCREEN`) chooses its default inside the argument list.  A
# table therefore describes *one* build.  The profile below is a Linux SDL2/TILES
# desktop build, the only one for which every value is statically known; the
# excluded registrations are listed in `meta.excluded` with their conditions, so
# the choice is visible rather than silently made.
# --------------------------------------------------------------------------
PROFILE = {
    "name": "linux-tiles",
    "defined": ["TILES", "LOCALIZE", "SDL_SOUND", "SDL_HINT_RENDER_BATCHING"],
    "undefined": [
        "__ANDROID__", "EMSCRIPTEN", "__EMSCRIPTEN__", "_MSC_VER", "_WIN32", "TUI",
    ],
}

# Integer constants the reference uses as registration arguments, and the header
# that defines them.  Resolved, not assumed: the value is evaluated from the
# header (`MAX_VIEW_DISTANCE = SEEX * HALF_MAPSIZE` = 60).
CONSTANT_HEADERS = ["src/map_scale_constants.h"]

# Slots the reference fills from runtime state.  Keyed by (option, slot); the
# expected source text is compared verbatim (modulo whitespace) so that a change
# in the reference is an error rather than a silent table update.
RUNTIME_SLOTS = {
    # (option, argument index after name/page/menu/tooltip):
    # (class, slot name, exact source text, why it cannot be known statically)
    ("SOUNDPACKS", 0): (
        "items", "items", "soundpacks",
        "build_soundpacks_list() scans the soundpacks directory (options.cpp:1306,1740)",
    ),
    ("SOUNDPACKS", 1): (
        "string", "default",
        'std::any_of( soundpacks.begin(), soundpacks.end(), []( const id_and_option & option ) { '
        'return option.first == "CC-Sounds"; } ) ? "CC-Sounds" : "basic"',
        "depends on which soundpacks are installed (options.cpp:1743-1746)",
    ),
    ("USE_LANG", 0): (
        "items", "items", "options_manager::get_lang_options()",
        "locale scan of the language files (options.cpp:1337)",
    ),
    ("USE_METRIC_SPEEDS", 1): (
        "string", "default",
        '( SystemLocale::UseMetricSystem().value_or( false ) ? "km/h" : "mph" )',
        "reads the OS locale (options.cpp:1809)",
    ),
    ("USE_METRIC_WEIGHTS", 1): (
        "string", "default",
        '( SystemLocale::UseMetricSystem().value_or( false ) ? "kg" : "lbs" )',
        "reads the OS locale (options.cpp:1815)",
    ),
    ("DISTANCE_UNITS", 1): (
        "string", "default",
        '( SystemLocale::UseMetricSystem().value_or( false ) ? "metric" : "imperial" )',
        "reads the OS locale (options.cpp:1826)",
    ),
    ("TILES", 0): (
        "items", "items", "build_tilesets_list()",
        "gfx directory scan (options.cpp:1290,2450)",
    ),
    ("DISTANT_TILES", 0): (
        "items", "items", "build_tilesets_list()",
        "gfx directory scan (options.cpp:1290,2460)",
    ),
    ("OVERMAP_TILES", 0): (
        "items", "items", "om_tilesets",
        "filtered gfx directory scan (options.cpp:2486-2497)",
    ),
    ("DISPLAY", 0): (
        "items", "items", "display_list",
        "cata_tiles::build_display_list() probes SDL video displays (options.cpp:2650)",
    ),
    ("DISPLAY", 1): (
        "string", "default", "display_list.front().first",
        "first detected display (options.cpp:2654)",
    ),
    ("RENDERER", 0): (
        "items", "items", "renderer_list",
        "cata_tiles::build_renderer_list() (options.cpp:2677)",
    ),
    ("RENDERER", 1): (
        "string", "default", "default_renderer",
        "first renderer, or direct3d11 on Windows (options.cpp:2678-2685)",
    ),
}

HIDE_FLAGS = {
    "COPT_NO_HIDE": "no_hide",
    "COPT_SDL_HIDE": "sdl_hide",
    "COPT_CURSES_HIDE": "curses_hide",
    "COPT_POSIX_CURSES_HIDE": "posix_curses_hide",
    "COPT_NO_SOUND_HIDE": "no_sound_hide",
    "COPT_ALWAYS_HIDE": "always_hide",
}

# Defaults applied by the `add` overloads when the caller omits them
# (options.h:250-283).
DEFAULT_FORMAT = {"int": "%i", "float": "%.2f"}

EXTERNAL_PAGE = "external_options"
# add_external() gives an int option the whole int range and a float option the
# whole float range (options.cpp:322-334); `int` is 32 bits wide in the reference.
EXTERNAL_INT_MIN = -2**31
EXTERNAL_INT_MAX = 2**31 - 1
FLOAT_MAX = struct.unpack("f", b"\xff\xff\x7f\x7f")[0]
EXTERNAL_FLOAT_MIN = -FLOAT_MAX
EXTERNAL_FLOAT_MAX = FLOAT_MAX


class ExtractionError(Exception):
    """A registration the script could not parse.  Never recovered from."""


def fail(msg):
    raise ExtractionError(msg)


# --------------------------------------------------------------------------
# Lexing helpers.  All of them preserve offsets so that line numbers stay
# meaningful after comments and dead preprocessor branches are blanked out.
# --------------------------------------------------------------------------
def strip_comments(text):
    """Replace comments with spaces, keeping every offset and newline intact."""
    out = list(text)
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                out[i] = " "
                i += 1
        elif c == "/" and i + 1 < n and text[i + 1] == "*":
            out[i] = out[i + 1] = " "
            i += 2
            while i + 1 < n and not (text[i] == "*" and text[i + 1] == "/"):
                if text[i] != "\n":
                    out[i] = " "
                i += 1
            if i + 1 < n:
                out[i] = out[i + 1] = " "
                i += 2
        elif c == '"':
            i += 1
            while i < n and text[i] != '"':
                if text[i] == "\\":
                    i += 1
                i += 1
            i += 1
        elif c == "'":
            i += 1
            while i < n and text[i] != "'":
                if text[i] == "\\":
                    i += 1
                i += 1
            i += 1
        else:
            i += 1
    return "".join(out)


PP_TOKEN = re.compile(r"\s*(?:(\|\|)|(&&)|(!)|(\()|(\))|(defined)|([A-Za-z_]\w*)|(\d+))")


def eval_condition(expr, defined):
    """Evaluate a `#if` condition: `defined`, `!`, `&&`, `||`, parens, ints."""
    tokens = []
    pos = 0
    expr = expr.strip()
    while pos < len(expr):
        m = PP_TOKEN.match(expr, pos)
        if not m:
            fail(f"cannot tokenise preprocessor condition {expr!r} at {expr[pos:]!r}")
        pos = m.end()
        if m.group(1):
            tokens.append(("||", None))
        elif m.group(2):
            tokens.append(("&&", None))
        elif m.group(3):
            tokens.append(("!", None))
        elif m.group(4):
            tokens.append(("(", None))
        elif m.group(5):
            tokens.append((")", None))
        elif m.group(6):
            tokens.append(("defined", None))
        elif m.group(7):
            tokens.append(("ident", m.group(7)))
        elif m.group(8):
            tokens.append(("num", int(m.group(8))))
    idx = 0

    def peek():
        return tokens[idx][0] if idx < len(tokens) else None

    def take(kind):
        nonlocal idx
        if idx >= len(tokens) or tokens[idx][0] != kind:
            fail(f"malformed preprocessor condition {expr!r}")
        idx += 1

    def parse_or():
        val = parse_and()
        while peek() == "||":
            take("||")
            rhs = parse_and()
            val = val or rhs
        return val

    def parse_and():
        val = parse_unary()
        while peek() == "&&":
            take("&&")
            rhs = parse_unary()
            val = val and rhs
        return val

    def parse_unary():
        if peek() == "!":
            take("!")
            return not parse_unary()
        return parse_atom()

    def parse_atom():
        kind = peek()
        if kind == "(":
            take("(")
            val = parse_or()
            take(")")
            return val
        if kind == "defined":
            take("defined")
            opened = peek() == "("
            if opened:
                take("(")
            if peek() != "ident":
                fail(f"malformed defined() in {expr!r}")
            name = tokens[idx][1]
            take("ident")
            if opened:
                take(")")
            return name in defined
        if kind == "num":
            val = tokens[idx][1]
            take("num")
            return val != 0
        if kind == "ident":
            name = tokens[idx][1]
            take("ident")
            return name in defined
        fail(f"malformed preprocessor condition {expr!r}")

    val = parse_or()
    if idx != len(tokens):
        fail(f"trailing tokens in preprocessor condition {expr!r}")
    return val


def apply_preprocessor(text, defined):
    """Blank out the lines `defined` excludes.

    Returns the filtered text and, for every excluded line, the conditions that
    excluded it, so that an excluded registration can be reported with its guard
    rather than silently dropped.
    """
    lines = text.split("\n")
    out = []
    stack = []  # {"cond": text, "value": bool}
    excluded = {}
    for number, line in enumerate(lines, 1):
        directive = re.match(r"\s*#\s*(\w+)(.*)$", line)
        if not directive:
            active = all(frame["value"] for frame in stack)
            out.append(line if active else " " * len(line))
            false = [frame["cond"] for frame in stack if not frame["value"]]
            if false:
                excluded[number] = false
            continue
        name, rest = directive.group(1), directive.group(2).strip()
        parent = all(frame["value"] for frame in stack)
        if name in ("if", "ifdef", "ifndef"):
            if name == "ifdef":
                rest = f"defined({rest})"
            elif name == "ifndef":
                rest = f"!defined({rest})"
            stack.append({"cond": rest, "value": parent and eval_condition(rest, defined)})
        elif name in ("else", "elif"):
            if not stack:
                fail(f"options.cpp:{number}: #{name} without #if")
            frame = stack[-1]
            parent_above = all(frame["value"] for frame in stack[:-1])
            negated = "!(" + frame["cond"] + ")" if name == "else" else rest
            frame["cond"] = negated
            frame["value"] = parent_above and not frame["value"] \
                if name == "else" else parent_above and eval_condition(rest, defined)
        elif name == "endif":
            if not stack:
                fail(f"options.cpp:{number}: #endif without #if")
            stack.pop()
        out.append(" " * len(line))
    if stack:
        fail(f"options.cpp: unterminated #if: {stack[-1]['cond']}")
    return "\n".join(out), excluded


class Source:
    """Comment-stripped, preprocessor-filtered view of one file."""

    def __init__(self, text, defined=None):
        stripped = strip_comments(text)
        if defined is None:
            self.text = stripped
            self.excluded_lines = {}
        else:
            self.text, self.excluded_lines = apply_preprocessor(stripped, defined)
        self.line_starts = [0]
        for match in re.finditer("\n", self.text):
            self.line_starts.append(match.end())

    def line_of(self, offset):
        lo, hi = 0, len(self.line_starts) - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if self.line_starts[mid] <= offset:
                lo = mid
            else:
                hi = mid - 1
        return lo + 1


def matching(text, start, opener, closer):
    """Offset of the `closer` that balances the `opener` at `start`."""
    depth = 0
    i = start
    while i < len(text):
        c = text[i]
        if c == '"':
            i += 1
            while i < len(text) and text[i] != '"':
                if text[i] == "\\":
                    i += 1
                i += 1
        elif c == "'":
            i += 1
            while i < len(text) and text[i] != "'":
                if text[i] == "\\":
                    i += 1
                i += 1
        elif c == opener:
            depth += 1
        elif c == closer:
            depth -= 1
            if depth == 0:
                return i
        i += 1
    fail(f"unbalanced {opener}{closer} starting at offset {start}")


def split_top_level(text, base=0):
    """Split on top-level commas; returns [(text, absolute_start_offset)]."""
    parts = []
    depth = 0
    start = 0
    i = 0
    while i < len(text):
        c = text[i]
        if c == '"':
            i += 1
            while i < len(text) and text[i] != '"':
                if text[i] == "\\":
                    i += 1
                i += 1
        elif c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == "," and depth == 0:
            parts.append((text[start:i], base + start))
            start = i + 1
        i += 1
    parts.append((text[start:], base + start))
    return [(part.strip(), offset) for part, offset in parts]


def find_calls(source, name):
    """Every `name( ... )` call site: [(line, args, args_offset)]."""
    pattern = re.compile(r"(?<![\w.>:])" + name + r"\s*\(")
    text = source.text
    calls = []
    pos = 0
    while True:
        match = pattern.search(text, pos)
        if not match:
            return calls
        open_paren = match.end() - 1
        close_paren = matching(text, open_paren, "(", ")")
        inner_start = open_paren + 1
        args = split_top_level(text[inner_start:close_paren], inner_start)
        calls.append((source.line_of(match.start()), args, match.start()))
        pos = close_paren + 1


# --------------------------------------------------------------------------
# Literals.
# --------------------------------------------------------------------------
def unescape(body):
    out = []
    i = 0
    while i < len(body):
        c = body[i]
        if c == "\\" and i + 1 < len(body):
            nxt = body[i + 1]
            mapping = {"n": "\n", "t": "\t", "r": "\r", "0": "\0", "\\": "\\", '"': '"',
                       "'": "'", "?": "?"}
            out.append(mapping.get(nxt, nxt))
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


STRING_LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')


def parse_string(text):
    """A C++ string literal, including adjacent-literal concatenation."""
    pos = 0
    pieces = []
    while pos < len(text):
        while pos < len(text) and text[pos] in " \t\n":
            pos += 1
        match = STRING_LITERAL.match(text, pos)
        if not match:
            return None
        pieces.append(unescape(match.group(1)))
        pos = match.end()
    if not pieces:
        return None
    return "".join(pieces)


INT_LITERAL = re.compile(r"[-+]?\d+")
FLOAT_LITERAL = re.compile(r"[-+]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][-+]?\d+)?[fF]?")


def parse_int(text):
    text = text.strip()
    if INT_LITERAL.fullmatch(text):
        return int(text)
    return None


def parse_float(text):
    text = text.strip()
    if FLOAT_LITERAL.fullmatch(text) and ("." in text or text.lower().endswith("f")):
        value = float(text.rstrip("fF"))
        # The reference stores float options as 32-bit floats, so widen the
        # narrowed value: 0.01 is 0.009999999776482582 as an option value.
        return widen_f32(value)
    return None


def widen_f32(value):
    """The reference stores float options as 32-bit floats; widen, don't round."""
    return struct.unpack("f", struct.pack("f", value))[0]


def parse_bool(text):
    text = text.strip()
    if text == "true":
        return True
    if text == "false":
        return False
    return None


def parse_hide(text):
    name = HIDE_FLAGS.get(text.strip())
    if name is None:
        fail(f"unknown visibility flag {text.strip()!r}")
    return name


# --------------------------------------------------------------------------
# Integer constants from headers (MAX_VIEW_DISTANCE and friends).
# --------------------------------------------------------------------------
def load_constants(ref):
    values = {}
    for relative in CONSTANT_HEADERS:
        path = ref / relative
        if not path.exists():
            fail(f"missing constant header {path}")
        text = strip_comments(path.read_text(errors="replace"))
        for line in text.split("\n"):
            match = re.match(r"\s*constexpr\s+int\s+([A-Za-z_]\w*)\s*=\s*(.+?);\s*$", line)
            if not match:
                continue
            name, expr = match.group(1), match.group(2)
            value = eval_int_expr(expr, values)
            if value is None:
                fail(f"{relative}: cannot evaluate constexpr {name} = {expr}")
            values[name] = value
    return values


def eval_int_expr(expr, values):
    expr = expr.strip()
    while expr.startswith("static_cast<int>"):
        expr = expr[len("static_cast<int>"):].strip()
    try:
        tokens = re.findall(r"[A-Za-z_]\w*|\d+|[()+\-*/]", expr)
    except Exception:
        return None
    if not tokens:
        return None
    pos = 0

    def atom():
        nonlocal pos
        if pos >= len(tokens):
            return None
        token = tokens[pos]
        if token == "(":
            pos += 1
            value = additive()
            if pos >= len(tokens) or tokens[pos] != ")":
                return None
            pos += 1
            return value
        if token.lstrip("-+").isdigit():
            pos += 1
            return int(token)
        if token in values:
            pos += 1
            return values[token]
        return None

    def multiplicative():
        nonlocal pos
        value = atom()
        while value is not None and pos < len(tokens) and tokens[pos] in "*/":
            op = tokens[pos]
            pos += 1
            rhs = atom()
            if rhs is None:
                return None
            if op == "*":
                value *= rhs
            else:
                if rhs == 0:
                    return None
                value //= rhs
        return value

    def additive():
        nonlocal pos
        value = multiplicative()
        while value is not None and pos < len(tokens) and tokens[pos] in "+-":
            op = tokens[pos]
            pos += 1
            rhs = multiplicative()
            if rhs is None:
                return None
            value = value + rhs if op == "+" else value - rhs
        return value

    value = additive()
    if pos != len(tokens):
        return None
    return value


# --------------------------------------------------------------------------
# Value classification: each argument becomes a token with a class, so the
# overload is chosen from the reference's declared parameter lists rather than
# guessed from position.
# --------------------------------------------------------------------------
def literal_of(text, constants):
    """The literal an argument spells out, or None if it is not a literal."""
    stripped = text.strip()
    string = parse_string(stripped)
    if string is not None:
        return string
    if parse_bool(stripped) is not None:
        return parse_bool(stripped)
    integer = parse_int(stripped)
    if integer is not None:
        return integer
    real = parse_float(stripped)
    if real is not None:
        return real
    if re.fullmatch(r"[A-Za-z_]\w*", stripped) and stripped in constants:
        return constants[stripped]
    return None


def brace_item_class(text):
    """`items_int` for `{ {0, ...}, ... }`, `items_str` for `{ {"a", ...}, ... }`."""
    stripped = text.strip()
    close = matching(stripped, 0, "{", "}")
    if close != len(stripped) - 1:
        fail(f"braced argument has trailing text: {stripped[:60]!r}")
    inner = stripped[1:close]
    for element, _offset in split_top_level(inner):
        if not element:
            continue
        element_close = matching(element, 0, "{", "}")
        first = split_top_level(element[1:element_close])[0][0]
        if parse_string(first) is not None:
            return "items_str"
        if parse_int(first) is not None:
            return "items_int"
        fail(f"cannot read the key of braced item {element[:60]!r}")
    fail(f"empty braced item list: {stripped[:60]!r}")


def ledger_node(name, index, text):
    """The recorded runtime slot, after checking the reference still agrees."""
    entry = RUNTIME_SLOTS.get((name, index))
    if entry is None:
        return None
    kind, slot, expected, note = entry
    normalised = " ".join(text.split())
    if normalised != expected:
        fail(
            f"{name} argument {index} ({slot}): the reference expression changed\n"
            f"  ledger: {expected}\n  source: {normalised}\n"
            "  re-read the reference, then update RUNTIME_SLOTS"
        )
    return {
        "kind": "runtime",
        "expr": normalised,
        "class": kind,
        "slot": slot,
        "note": note,
    }


def arg_class(text, name, index, constants):
    """The token class of one add() argument, for overload selection."""
    stripped = text.strip()
    if stripped in HIDE_FLAGS:
        return "hide"
    if re.fullmatch(r'"%[^"]*"', stripped):
        return "format"
    if stripped.startswith("{"):
        return brace_item_class(stripped)
    literal = literal_of(text, constants)
    if literal is not None:
        if isinstance(literal, bool):
            return "bool"
        if isinstance(literal, str):
            return "str"
        if isinstance(literal, int):
            return "int"
        return "float"
    node = ledger_node(name, index, text)
    if node is not None:
        return "runtime:" + node["class"]
    if re.fullmatch(r"[A-Za-z_]\w*", stripped):
        fail(
            f"{name} argument {index}: unresolved identifier {stripped!r}: not a constant in "
            f"{CONSTANT_HEADERS} and not a RUNTIME_SLOTS entry"
        )
    fail(
        f"{name} argument {index}: unresolved expression {stripped[:80]!r}. Either the "
        "reference computes it at runtime (add it to RUNTIME_SLOTS with the reason) or this "
        "parser does not understand it."
    )


def value_node(text, name, index, role, constants):
    literal = literal_of(text, constants)
    if literal is not None:
        return {"kind": "literal", "value": literal}
    node = ledger_node(name, index, text)
    if node is None:
        arg_class(text, name, index, constants)  # raises with the reason
        fail(f"{name} argument {index}: cannot be read as {role}")
    return {"kind": "runtime", "expr": node["expr"], "class": node["class"], "note": node["note"]}


def items_node(text, name, index, constants):
    stripped = text.strip()
    if stripped.startswith("{"):
        inner = stripped[1:matching(stripped, 0, "{", "}")]
        items = []
        for element, _offset in split_top_level(inner):
            if not element:
                continue
            element_close = matching(element, 0, "{", "}")
            fields = split_top_level(element[1:element_close])
            if len(fields) != 2:
                fail(f"{name}: braced item has {len(fields)} fields, expected 2: {element[:60]!r}")
            key = literal_of(fields[0][0], constants)
            if isinstance(key, str):
                items.append({"value": key})
            elif isinstance(key, int) and not isinstance(key, bool):
                items.append({"value": key})
            else:
                fail(f"{name}: cannot read braced item key {fields[0][0]!r}")
        if not items:
            fail(f"{name}: empty item list")
        return {"kind": "literal", "items": items}
    node = ledger_node(name, index, text)
    if node is None or node["class"] != "items":
        arg_class(text, name, index, constants)  # raises with the reason
        fail(f"{name} argument {index}: expected an item list, got {stripped[:60]!r}")
    return {"kind": "runtime", "expr": node["expr"], "class": "items", "note": node["note"]}


def group_pages(source):
    """Span -> page id, for `add_option_group` lambdas that pass `page_id` on."""
    spans = []
    for _line, args, _offset in find_calls(source, "add_option_group"):
        if len(args) < 3:
            fail("add_option_group with fewer than three arguments")
        page = parse_string(args[0][0])
        if page is None:
            fail(f"add_option_group page is not a literal: {args[0][0]!r}")
        body_text, body_offset = args[2]
        brace = body_text.find("{")
        if brace < 0:
            fail("add_option_group lambda has no body")
        close = matching(body_text, brace, "{", "}")
        spans.append((body_offset + brace, body_offset + close, page))
    spans.sort()
    return spans


def resolve_page(text, offset, spans):
    stripped = text.strip()
    page = parse_string(stripped)
    if page is not None:
        return page
    if stripped != "page_id":
        fail(f"unresolved page expression {stripped!r}")
    best = None
    for start, end, page_id in spans:
        if start < offset < end and (best is None or start > best[0]):
            best = (start, end, page_id)
    if best is None:
        fail("`page_id` used outside an add_option_group lambda")
    return best[2]


def check_text_arg(text, name, role):
    stripped = text.strip()
    if stripped in ("translation()", "NO_TRANSLATION"):
        return
    if stripped.startswith("to_translation(") or stripped.startswith("translation("):
        return
    fail(f"{name}: {role} argument is not a translation: {stripped[:60]!r}")


NUM = {"int", "float"}
BOOL = {"bool", "runtime:bool"}
TEXT = {"str", "runtime:string"}
ITEMS_STR = {"items_str", "runtime:items"}
ITEMS_INT = {"items_int"}
HIDE = {"hide"}
FORMAT = {"format"}
SIGNATURES = [
    # One entry per accepted argument sequence of each `add` overload
    # (options.h:250-283).  A parameter accepts any class in its set.
    ("bool", [BOOL]),
    ("bool", [BOOL, HIDE]),
    ("string_select", [ITEMS_STR, TEXT]),
    ("string_select", [ITEMS_STR, TEXT, HIDE]),
    ("string_input", [TEXT, {"int", "runtime:int"}]),
    ("string_input", [TEXT, {"int", "runtime:int"}, HIDE]),
    ("int", [{"int"}, {"int"}, {"int"}]),
    ("int", [{"int"}, {"int"}, {"int"}, HIDE]),
    ("int", [{"int"}, {"int"}, {"int"}, HIDE, FORMAT]),
    ("float", [NUM, NUM, NUM, NUM]),
    ("float", [NUM, NUM, NUM, NUM, HIDE]),
    ("float", [NUM, NUM, NUM, NUM, HIDE, FORMAT]),
    ("int_map", [ITEMS_INT, {"int"}, {"int"}]),
    ("int_map", [ITEMS_INT, {"int"}, {"int"}, HIDE]),
]


def match_overload(classes):
    """The single option type whose argument list accepts these classes."""
    matched = set()
    for option_type, params in SIGNATURES:
        if len(params) != len(classes):
            continue
        if all(actual in accepted for actual, accepted in zip(classes, params)):
            matched.add(option_type)
    return matched


def extract_cpp_option(source, line, args, spans, constants):
    if len(args) < 5:
        fail(f"options.cpp:{line}: an `add` call with only {len(args)} arguments")
    name = parse_string(args[0][0])
    if name is None:
        fail(f"options.cpp:{line}: option name is not a literal: {args[0][0]!r}")
    page = resolve_page(args[1][0], args[1][1], spans)
    check_text_arg(args[2][0], name, "menu text")
    check_text_arg(args[3][0], name, "tooltip")

    rest = args[4:]
    classes = [arg_class(text, name, index, constants)
               for index, (text, _offset) in enumerate(rest)]
    matches = match_overload(classes)
    if len(matches) != 1:
        fail(
            f"options.cpp:{line}: {name}: add() overload is "
            f"{'ambiguous' if matches else 'unknown'} for argument classes {classes}"
            + (f"; candidates {sorted(matches)}" if matches else "")
        )
    option_type = matches.pop()

    entry = {
        "name": name,
        "source": "options.cpp",
        "line": line,
        "page": page,
        "type": option_type,
        "hide": "no_hide",
        "in_page": True,
        "default": None,
        "initial": None,
        "min": None,
        "max": None,
        "step": None,
        "max_length": None,
        "format": None,
        "items": None,
    }
    findings = []

    def slot(index, role):
        return value_node(rest[index][0], name, index, role, constants)

    def as_int(node, what):
        if node["kind"] == "runtime":
            fail(f"{name}: {what} is a runtime expression, the int overload needs a number")
        value = node["value"]
        if isinstance(value, bool) or not isinstance(value, int):
            fail(f"{name}: {what} is {value!r}, which is not an int literal")
        return node

    def as_float(node, what):
        if node["kind"] == "runtime":
            fail(f"{name}: {what} is a runtime expression, the float overload needs a number")
        value = node["value"]
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            fail(f"{name}: {what} is {value!r}, which is not a number")
        return {"kind": "literal", "value": widen_f32(float(value))}

    def hide_flag(index):
        return parse_hide(rest[index][0])

    if option_type == "bool":
        entry["default"] = slot(0, "default")
        entry["initial"] = entry["default"]
        if len(rest) > 1:
            entry["hide"] = hide_flag(1)

    elif option_type == "string_select":
        entry["items"] = items_node(rest[0][0], name, 0, constants)
        entry["default"] = slot(1, "default")
        entry["initial"] = entry["default"]
        if len(rest) > 2:
            entry["hide"] = hide_flag(2)

    elif option_type == "string_input":
        entry["default"] = slot(0, "default")
        entry["initial"] = entry["default"]
        entry["max_length"] = slot(1, "max_length")
        if len(rest) > 2:
            entry["hide"] = hide_flag(2)

    elif option_type == "int":
        entry["min"] = as_int(slot(0, "min"), "min")
        entry["max"] = as_int(slot(1, "max"), "max")
        entry["default"] = as_int(slot(2, "default"), "default")
        entry["initial"] = entry["default"]
        entry["format"] = {"kind": "literal", "value": DEFAULT_FORMAT["int"]}
        for text, _offset in rest[3:]:
            stripped = text.strip()
            if stripped in HIDE_FLAGS:
                entry["hide"] = parse_hide(stripped)
            elif re.fullmatch(r'"%[^"]*"', stripped):
                entry["format"] = {"kind": "literal", "value": parse_string(stripped)}
            else:
                fail(f"{name}: unexpected int argument {stripped[:40]!r}")

    elif option_type == "float":
        entry["min"] = as_float(slot(0, "min"), "min")
        entry["max"] = as_float(slot(1, "max"), "max")
        entry["default"] = as_float(slot(2, "default"), "default")
        entry["step"] = as_float(slot(3, "step"), "step")
        entry["initial"] = entry["default"]
        entry["format"] = {"kind": "literal", "value": DEFAULT_FORMAT["float"]}
        for text, _offset in rest[4:]:
            stripped = text.strip()
            if stripped in HIDE_FLAGS:
                entry["hide"] = parse_hide(stripped)
            elif re.fullmatch(r'"%[^"]*"', stripped):
                entry["format"] = {"kind": "literal", "value": parse_string(stripped)}
            else:
                fail(f"{name}: unexpected float argument {stripped[:40]!r}")

    elif option_type == "int_map":
        entry["items"] = items_node(rest[0][0], name, 0, constants)
        entry["initial"] = as_int(slot(1, "initial"), "initial")
        entry["default"] = as_int(slot(2, "default"), "default")
        if len(rest) > 3:
            entry["hide"] = hide_flag(3)

    # Registration-time validation, as the reference does it (options.cpp:432-456,
    # 357-370, 386-395, 489-503).
    if entry["items"] is not None and entry["items"]["kind"] == "literal":
        choices = [item["value"] for item in entry["items"]["items"]]
    else:
        choices = None
    if option_type in ("int", "float"):
        low, high = entry["min"], entry["max"]
        if low["kind"] == "literal" and high["kind"] == "literal":
            if low["value"] > high["value"]:
                findings.append({
                    "kind": "min_above_max",
                    "option": name,
                    "detail": f"add() clamps max to min: {low['value']}..{high['value']}",
                })
                entry["max"] = {"kind": "literal", "value": low["value"]}
                high = entry["max"]
        if entry["default"]["kind"] == "literal" and low["kind"] == "literal" \
                and high["kind"] == "literal":
            value = entry["default"]["value"]
            if value < low["value"] or value > high["value"]:
                findings.append({
                    "kind": "default_out_of_range",
                    "option": name,
                    "detail": (
                        f"default {value!r} outside {low['value']!r}..{high['value']!r}; "
                        "add() replaces it with the minimum"
                    ),
                })
                entry["default"] = {"kind": "literal", "value": low["value"]}
                entry["initial"] = entry["default"]
    if option_type == "string_select" and entry["default"]["kind"] == "literal" \
            and choices is not None:
        if entry["default"]["value"] not in choices:
            findings.append({
                "kind": "default_not_an_item",
                "option": name,
                "detail": f"default {entry['default']['value']!r} not in items; add() uses items[0]",
            })
            entry["default"] = {"kind": "literal", "value": choices[0]}
            entry["initial"] = entry["default"]
    if option_type == "string_input" and entry["default"]["kind"] == "literal" \
            and entry["max_length"]["kind"] == "literal":
        limit = entry["max_length"]["value"]
        if limit > 0 and len(entry["default"]["value"]) > limit:
            findings.append({
                "kind": "default_truncated",
                "option": name,
                "detail": f"default truncated to max_length {limit}",
            })
            entry["default"] = {"kind": "literal", "value": entry["default"]["value"][:limit]}
            entry["initial"] = entry["default"]
    if option_type == "int_map" and choices is not None:
        for role in ("initial", "default"):
            node = entry[role]
            if node["kind"] == "literal" and node["value"] not in choices:
                findings.append({
                    "kind": f"{role}_not_an_item",
                    "option": name,
                    "detail": f"{role} {node['value']!r} not in the map; add() uses the first key",
                })
                entry[role] = {"kind": "literal", "value": choices[0]}
    return entry, findings


def extract_external_options(ref, constants):
    path = ref / "data/core/external_options.json"
    if not path.exists():
        fail(f"missing {path}")
    raw = path.read_text()
    data = json.loads(raw)
    lines = {}
    for number, text in enumerate(raw.split("\n"), 1):
        match = re.search(r'"name"\s*:\s*"([^"]+)"', text)
        if match:
            lines.setdefault(match.group(1), number)
    entries = []
    findings = []
    skipped = []
    seen = set()
    for item in data:
        if item.get("type") != "EXTERNAL_OPTION":
            fail(f"{path}: entry of unknown type {item.get('type')!r}")
        name = item["name"]
        line = lines.get(name, 0)
        if item.get("stub", False):
            skipped.append(name)
            continue
        if name in seen:
            findings.append({
                "kind": "duplicate_external",
                "option": name,
                "detail": "declared twice in external_options.json",
            })
        seen.add(name)
        stype = item.get("stype")
        if stype not in ("bool", "int", "float", "string_input"):
            fail(
                f"{name}: external stype {stype!r} is not one the reference accepts for "
                "an option it registers itself (worldfactory.cpp:2077-2092)"
            )
        value = item.get("value")
        option_type = stype
        entry = {
            "name": name,
            "source": "external_options.json",
            "line": line,
            "page": EXTERNAL_PAGE,
            "type": option_type,
            "hide": "always_hide",
            "in_page": False,
            "default": None,
            "initial": {"kind": "literal", "value": value},
            "min": None,
            "max": None,
            "step": None,
            "max_length": None,
            "format": None,
            "items": None,
        }
        # add_external() gives every external option the value type's zero
        # defaults and then load_external_option() applies the declared value as
        # the *current* value; the reset target stays at the zero default
        # (options.cpp:311-348, worldfactory.cpp:2069-2088).
        if stype == "bool":
            entry["default"] = {"kind": "literal", "value": False}
        elif stype == "int":
            entry["default"] = {"kind": "literal", "value": 0}
            entry["min"] = {"kind": "literal", "value": EXTERNAL_INT_MIN}
            entry["max"] = {"kind": "literal", "value": EXTERNAL_INT_MAX}
            entry["format"] = {"kind": "literal", "value": DEFAULT_FORMAT["int"]}
        elif stype == "float":
            entry["default"] = {"kind": "literal", "value": 0.0}
            entry["min"] = {"kind": "literal", "value": EXTERNAL_FLOAT_MIN}
            entry["max"] = {"kind": "literal", "value": EXTERNAL_FLOAT_MAX}
            entry["step"] = {"kind": "literal", "value": 1.0}
            entry["format"] = {"kind": "literal", "value": DEFAULT_FORMAT["float"]}
            entry["initial"] = {"kind": "literal", "value": widen_f32(float(value))}
        elif stype == "string_input":
            entry["default"] = {"kind": "literal", "value": ""}
        entries.append(entry)
    return entries, findings, skipped


def extract_migrations(source):
    """The hardcoded option rename/value maps (options.cpp:196-207, 3999-4030)."""
    text = source.text
    anchor = text.find("get_migrated_options()")
    if anchor < 0:
        fail("cannot find get_migrated_options()")
    start = text.find("{", text.find("=", anchor))
    end = matching(text, start, "{", "}")
    body = text[start + 1:end]
    migrations = []
    for element, _offset in split_top_level(body):
        element = element.strip()
        if not element:
            continue
        if not element.startswith("{"):
            fail(f"migration entry is not a pair: {element!r}")
        close = matching(element, 0, "{", "}")
        old = parse_string(split_top_level(element[1:close])[0][0])
        target = split_top_level(element[1:close])[1][0]
        target_close = matching(target, 0, "{", "}")
        target_fields = split_top_level(target[1:target_close])
        new = parse_string(target_fields[0][0])
        values = {}
        mapping = target_fields[1][0].strip()
        if mapping:
            mapping_close = matching(mapping, 0, "{", "}")
            for pair, _o in split_top_level(mapping[1:mapping_close]):
                pair = pair.strip()
                if not pair:
                    continue
                pair_close = matching(pair, 0, "{", "}")
                fields = split_top_level(pair[1:pair_close])
                values[parse_string(fields[0][0])] = parse_string(fields[1][0])
        migrations.append({"old": old, "new": new, "values": values})

    removed = []
    anchor = text.find("removed_options")
    if anchor < 0:
        fail("cannot find removed_options in deserialize()")
    start = text.find("{", anchor)
    end = matching(text, start, "{", "}")
    for element, _offset in split_top_level(text[start + 1:end]):
        name = parse_string(element)
        if name is None:
            fail(f"removed option name is not a literal: {element!r}")
        removed.append(name)
    return migrations, removed


def build_table(ref):
    cpp = ref / "src/options.cpp"
    if not cpp.exists():
        fail(f"missing {cpp}")
    raw = cpp.read_text(errors="replace")
    defined = set(PROFILE["defined"])
    source = Source(raw, defined)
    constants = load_constants(ref)
    spans = group_pages(source)

    entries = []
    findings = []
    seen = {}
    for line, args, offset in find_calls(source, "add"):
        entry, entry_findings = extract_cpp_option(source, line, args, spans, constants)
        findings.extend(entry_findings)
        if entry["name"] in seen:
            findings.append({
                "kind": "duplicate_registration",
                "option": entry["name"],
                "detail": (
                    f"registered twice in this build profile: options.cpp:{seen[entry['name']]} "
                    f"and options.cpp:{line}; the reference overwrites the first "
                    "(options.cpp:373, options[sNameIn] = thisOpt)"
                ),
            })
        seen[entry["name"]] = line
        entries.append(entry)

    enabled_for_json = [parse_string(args[0][0]) for _line, args, _offset
                        in find_calls(source, "enable_json") if args]
    external, external_findings, skipped = extract_external_options(ref, constants)
    findings.extend(external_findings)
    shadowed = []
    for entry in external:
        if entry["name"] in seen:
            shadowed.append({
                "name": entry["name"],
                "cpp_line": seen[entry["name"]],
                "detail": (
                    "declared in external_options.json as well, but load_external_option() "
                    "keeps the options.cpp registration and only applies the declared value "
                    "(worldfactory.cpp:2069-2071)"
                ),
            })
    entries.extend(external)
    entries.sort(key=lambda item: item["name"])

    by_name = {}
    for entry in entries:
        by_name.setdefault(entry["name"], []).append(entry)
    for name in enabled_for_json:
        found = by_name.get(name)
        if not found:
            findings.append({
                "kind": "enable_json_without_option",
                "option": name,
                "detail": "enable_json() names an option that is never registered",
            })
            continue
        for entry in found:
            if entry["type"] not in ("string_select",):
                findings.append({
                    "kind": "add_value_target_is_not_a_string_select",
                    "option": name,
                    "detail": (
                        f"enable_json() + add_value() only extend a string_select, but this "
                        f"option is registered as {entry['type']} ({entry['source']}:"
                        f"{entry['line']}), so add_value() silently does nothing "
                        "(options.cpp:257-272)"
                    ),
                })

    excluded = find_excluded(raw, defined, {entry["name"] for entry in entries})
    variants = find_variants(raw, defined, entries)

    by_type = {}
    by_source = {}
    for entry in entries:
        by_type[entry["type"]] = by_type.get(entry["type"], 0) + 1
        by_source[entry["source"]] = by_source.get(entry["source"], 0) + 1

    runtime = [
        {"option": entry["name"], "slot": slot_name, "expr": node["expr"], "note": node["note"]}
        for entry in entries
        for slot_name, node in (
            ("items", entry["items"]),
            ("default", entry["default"]),
            ("initial", entry["initial"]),
            ("min", entry["min"]),
            ("max", entry["max"]),
            ("step", entry["step"]),
            ("max_length", entry["max_length"]),
        )
        if node is not None and node.get("kind") == "runtime"
    ]
    # Two different facts, kept apart: an option can have a literal start value
    # and still have a runtime *item* list, and only the former starts unset.
    start_unset = sorted({entry["name"] for entry in entries
                          if entry["initial"]["kind"] == "runtime"})
    items_runtime = sorted({entry["name"] for entry in entries
                            if entry["items"] is not None
                            and entry["items"]["kind"] == "runtime"})
    migrations, removed = extract_migrations(source)

    meta = {
        "generated_by": "tools/gen_options.py",
        "reference": {
            "checkout": "references/Cataclysm-DDA",
            "commit": reference_commit(ref),
            "options_cpp": "src/options.cpp",
            "options_h": "src/options.h",
            "external_options_json": "data/core/external_options.json",
            "constants": CONSTANT_HEADERS,
        },
        "build_profile": PROFILE,
        "counts": {
            "total": len(entries),
            "by_type": dict(sorted(by_type.items())),
            "by_source": dict(sorted(by_source.items())),
            "cpp_registrations": sum(1 for entry in entries if entry["source"] == "options.cpp"),
            "external_registrations": len(external),
            "external_stubs_skipped": sorted(skipped),
        },
        "options_starting_unset": start_unset,
        "options_with_runtime_item_lists": items_runtime,
        "runtime_expressions": runtime,
        "findings": findings,
        "excluded_by_profile": excluded,
        "profile_variants": variants,
        "enable_json": enabled_for_json,
        "external_shadowed_by_cpp": shadowed,
        "migrations": migrations,
        "removed_options": removed,
        "notes": [
            "float values are the reference's 32-bit floats, widened exactly "
            "(0.01f is 0.009999999776482582)",
            "`initial` is the value a fresh install starts with, `default` is what "
            "setValue() falls back to when it rejects an input; they differ for "
            "external_options.json entries (options.cpp:311-348, worldfactory.cpp:2080-2088)",
            "string_select item *texts* are translations and are not recorded; only "
            "the item values are, which is what membership checks need",
        ],
    }
    return {"meta": meta, "options": entries}


def find_excluded(raw, defined, present):
    """Registrations the profile removes, with the guard that removed them."""
    guarded = Source(raw, defined)
    excluded = []
    seen = set(present)
    for line, args, _offset in find_calls(Source(raw), "add"):
        if not args:
            continue
        name = parse_string(args[0][0])
        if name is None or name in seen:
            continue
        conditions = guarded.excluded_lines.get(line)
        if not conditions:
            continue
        seen.add(name)
        excluded.append({
            "name": name,
            "line": line,
            "condition": " && ".join(f"({condition})" for condition in conditions),
        })
    return excluded


def find_variants(raw, defined, entries):
    """Names registered at more than one site, and which site the profile keeps."""
    chosen = {(entry["name"], entry["line"]) for entry in entries
              if entry["source"] == "options.cpp"}
    sites = {}
    for line, args, _offset in find_calls(Source(raw), "add"):
        if not args:
            continue
        name = parse_string(args[0][0])
        if name is not None:
            sites.setdefault(name, []).append(line)
    variants = []
    for name in sorted(sites):
        lines = sorted(sites[name])
        if len(lines) < 2:
            continue
        variants.append({
            "name": name,
            "sites": [{"line": line, "in_profile": (name, line) in chosen} for line in lines],
        })
    return variants


def reference_commit(ref):
    try:
        result = subprocess.run(
            ["git", "-C", str(ref), "rev-parse", "HEAD"],
            capture_output=True, text=True, check=True,
        )
        return result.stdout.strip()
    except Exception:
        return "unknown"


def render(table):
    return json.dumps(table, indent=2, sort_keys=False, ensure_ascii=False) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ref", default=str(DEFAULT_REF))
    parser.add_argument("--out", default=str(DEFAULT_OUT))
    parser.add_argument("--check", action="store_true",
                        help="exit 1 if the checked-in table is stale")
    args = parser.parse_args()

    ref = pathlib.Path(args.ref)
    if not (ref / "src" / "options.cpp").exists():
        sys.exit(f"not a CDDA checkout: {ref}")
    try:
        table = build_table(ref)
    except ExtractionError as error:
        sys.exit(f"gen_options: {error}")

    text = render(table)
    out = pathlib.Path(args.out)
    if args.check:
        if not out.exists():
            sys.exit(f"gen_options: {out} is missing")
        if out.read_text() != text:
            sys.exit(f"gen_options: {out} is stale; re-run without --check")
        print(f"{out}: up to date ({table['meta']['counts']['total']} options)")
        return
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text)
    counts = table["meta"]["counts"]
    print(f"{out}: {counts['total']} options {counts['by_type']} "
          f"from {counts['by_source']}")


if __name__ == "__main__":
    main()
