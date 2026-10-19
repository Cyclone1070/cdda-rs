#!/usr/bin/env python3
"""Extract every message id the reference marks for translation.

Scans the pinned CDDA checkout for calls to the translation macro family
(``_( ... )``, ``pgettext( ... )``, ``n_gettext( ... )``, ``npgettext( ... )``,
``translate_marker( ... )``, ``translate_marker_context( ... )``) and writes one
entry per line to ``corpus/text/msgids.txt``.

The output is a *lookup key* list, because that is what the lookup layer in
``crates/core-types/src/strings.rs`` receives:

  * a line with no TAB is a bare ``msgid`` (``_`` / ``translate_marker`` /
    ``n_gettext`` singular and plural / ``npgettext`` singular and plural);
  * a line ``<context>\\t<msgid>`` is a ``pgettext``-style key. The reference
    builds that key as ``context + "\\004" + msgid`` (translation_manager_impl.cpp
    ``ConstructContextualQuery``); a TAB keeps the file plain text while staying
    unambiguous.

Backslash, TAB, CR and LF inside a message id are written as ``\\\\``, ``\\t``,
``\\r``, ``\\n`` so that one entry is always one line; that is the only escaping
in the file and it is exactly reversed by the reader in ``tests/strings.rs``.

Every argument that must be a string literal is one, otherwise the tool exits
non-zero with ``file:line`` -- that is the "fail loudly on a form it cannot
parse" contract. A macro call whose argument is *not* a literal (``_( name )``)
is not an error: xgettext cannot extract it either, so it is counted and
reported separately as ``nonliteral``.

Forms handled: adjacent literal concatenation (``_( "a" "b" )``), raw strings
with and without a delimiter (``R"( ... )"``, ``R"xy( ... )xy"``), all C escape
sequences, and the macros' own ``#define`` bodies (skipped, they are not calls).

Usage:  python3 tools/gen_msgids.py [--ref PATH] [--out PATH] [--quiet]
"""

import argparse
import pathlib
import re
import sys
from collections import Counter

DEFAULT_REF = pathlib.Path("references/Cataclysm-DDA")
DEFAULT_OUT = pathlib.Path("corpus/text/msgids.txt")

# macro name -> (index of the context argument or None, indices of the message
# id arguments). Every index but the trailing count expression of the plural
# macros must be a string literal; the count is an arbitrary expression and its
# tokens are ignored.
FAMILIES = {
    "_": (None, [0]),
    "translate_marker": (None, [0]),
    "pgettext": (0, [1]),
    "translate_marker_context": (0, [1]),
    "n_gettext": (None, [0, 1]),
    "npgettext": (0, [1, 2]),
}

# A plural macro's literal slots are *singular* then *plural*; both are message
# ids and both are emitted.
PLURAL_FAMILIES = {"n_gettext", "npgettext"}

IDENT_START = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_")
IDENT_BODY = IDENT_START | set("0123456789")
SIMPLE_ESCAPES = {
    "n": "\n",
    "t": "\t",
    "r": "\r",
    "0": "\0",
    "a": "\a",
    "b": "\b",
    "f": "\f",
    "v": "\v",
    "\\": "\\",
    "'": "'",
    '"': '"',
    "?": "?",
}


RAW_OPEN = re.compile(r'(?:u8|u|U|L)?R"([^ ()\\\t\v\f\n]{0,16})\(')
STRING_OPEN = re.compile(r'(u8|[uUL])?"')
MACRO_DEFINE = re.compile(
    r"#\s*define\s+(" + "|".join(re.escape(k) for k in FAMILIES) + r")\b")


class Failure(Exception):
    """A form the tool refuses to guess at."""


class Tok:
    """One lexical token: kind is 'id', 'str', 'punct' or 'other'."""

    __slots__ = ("kind", "text", "line", "wide")

    def __init__(self, kind, text, line, wide=False):
        self.kind = kind
        self.text = text
        self.line = line
        self.wide = wide

    def __repr__(self):
        return "Tok(%s,%r,%d)" % (self.kind, self.text, self.line)


def strip_comments_and_directives(src, path):
    """Blank out comments and translation-macro #defines, keeping line numbers.

    Line numbers must survive, so blanks are spaces plus the original newlines.
    A #define whose body spans continuation lines is blanked whole: those bodies
    are the macros' own definitions, not call sites, and would otherwise be
    reported as non-literal calls.
    """
    out = []
    i = 0
    n = len(src)
    line = 1
    at_line_start = True
    while i < n:
        ch = src[i]
        if ch == "\n":
            out.append(ch)
            line += 1
            i += 1
            at_line_start = True
            continue
        if ch in " \t\r":
            out.append(ch)
            i += 1
            continue
        if ch == "#" and at_line_start:
            # A preprocessor line is only special when it defines one of the
            # translation macros: that definition is not a call site and is
            # blanked whole, continuations included. Any other directive is
            # scanned like ordinary text, so a "//" comment on a directive line
            # is still blanked and cannot leak an apostrophe into a token.
            j = i
            while j < n:
                if src[j] == "\n" and src[j - 1] != "\\":
                    break
                j += 1
            text = src[i:j]
            if MACRO_DEFINE.match(text):
                out.append("".join("\n" if c == "\n" else " " for c in text))
                line += text.count("\n")
                i = j
                at_line_start = True
                continue
        at_line_start = False
        # A raw string literal is copied verbatim before any comment handling:
        # "//" inside R"( ... )" is literal text, not a comment.
        m = RAW_OPEN.match(src, i)
        if m:
            delim = m.group(1)
            body_start = m.end()
            terminator = ")" + delim + '"'
            body_end = src.find(terminator, body_start)
            if body_end < 0:
                raise Failure("%s:%d: unterminated raw string" % (path, line))
            end = body_end + len(terminator)
            chunk = src[i:end]
            out.append(chunk)
            line += chunk.count("\n")
            i = end
            continue
        if src.startswith("//", i):
            j = src.find("\n", i)
            if j < 0:
                j = n
            out.append(" " * (j - i))
            i = j
            continue
        if src.startswith("/*", i):
            j = src.find("*/", i + 2)
            if j < 0:
                raise Failure("%s:%d: unterminated block comment" % (path, line))
            chunk = src[i:j + 2]
            out.append("".join("\n" if c == "\n" else " " for c in chunk))
            line += chunk.count("\n")
            i = j + 2
            continue
        if ch == '"' or ch == "'":
            quote = ch
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == quote:
                    break
                if src[j] == "\n":
                    raise Failure("%s:%d: newline inside literal" % (path, line))
                j += 1
            if j >= n:
                raise Failure("%s:%d: unterminated literal" % (path, line))
            out.append(src[i:j + 1])
            i = j + 1
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def tokenize(src, path):
    """Split blanked source into identifier / literal / punctuation tokens."""
    toks = []
    i = 0
    n = len(src)
    line = 1
    while i < n:
        ch = src[i]
        if ch == "\n":
            line += 1
            i += 1
            continue
        if ch in " \t\r":
            i += 1
            continue
        # raw string literal, possibly with a filename-like prefix
        m = RAW_OPEN.match(src, i)
        if m:
            delim = m.group(1)
            body_start = m.end()
            terminator = ")" + delim + '"'
            body_end = src.find(terminator, body_start)
            if body_end < 0:
                raise Failure("%s:%d: unterminated raw string" % (path, line))
            body = src[body_start:body_end]
            toks.append(Tok("str", body, line))
            line += body.count("\n")
            i = body_end + len(terminator)
            continue
        # ordinary literal with optional encoding prefix
        m = STRING_OPEN.match(src, i)
        if m:
            prefix = m.group(1)
            start_line = line
            j = m.end()
            buf = []
            while True:
                if j >= n:
                    raise Failure("%s:%d: unterminated string literal" % (path, start_line))
                c = src[j]
                if c == "\\":
                    if j + 1 >= n:
                        raise Failure("%s:%d: dangling escape" % (path, start_line))
                    e = src[j + 1]
                    if e == "x":
                        k = j + 2
                        hexdigits = ""
                        while k < n and src[k] in "0123456789abcdefABCDEF":
                            hexdigits += src[k]
                            k += 1
                        if not hexdigits:
                            raise Failure("%s:%d: empty \\x escape" % (path, start_line))
                        buf.append(chr(int(hexdigits, 16)))
                        j = k
                        continue
                    if e in "01234567":
                        k = j + 1
                        oct_digits = ""
                        while k < n and len(oct_digits) < 3 and src[k] in "01234567":
                            oct_digits += src[k]
                            k += 1
                        buf.append(chr(int(oct_digits, 8)))
                        j = k
                        continue
                    if e == "u" or e == "U":
                        width = 4 if e == "u" else 8
                        digits = src[j + 2:j + 2 + width]
                        if len(digits) != width or any(c2 not in "0123456789abcdefABCDEF" for c2 in digits):
                            raise Failure("%s:%d: malformed \\%s escape" % (path, start_line, e))
                        buf.append(chr(int(digits, 16)))
                        j += 2 + width
                        continue
                    if e in SIMPLE_ESCAPES:
                        buf.append(SIMPLE_ESCAPES[e])
                        j += 2
                        continue
                    raise Failure("%s:%d: unknown escape \\%s" % (path, start_line, e))
                if c == '"':
                    j += 1
                    break
                if c == "\n":
                    raise Failure("%s:%d: newline inside string literal" % (path, start_line))
                buf.append(c)
                j += 1
            value = "".join(buf)
            toks.append(Tok("str", value, start_line, wide=prefix not in (None, "u8")))
            line += src[i:j].count("\n")
            i = j
            continue
        if ch == "'":
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == "'":
                    break
                j += 1
            toks.append(Tok("other", src[i:j + 1], line))
            i = j + 1
            continue
        if ch in IDENT_START:
            j = i
            while j < n and src[j] in IDENT_BODY:
                j += 1
            toks.append(Tok("id", src[i:j], line))
            i = j
            continue
        toks.append(Tok("punct", ch, line))
        i += 1
    return toks


def split_args(toks, start, path):
    """Split the parenthesised argument list starting at toks[start] == '('.

    Returns (list of token lists, index of the matching ')').
    """
    if toks[start].text != "(":
        raise Failure("%s:%d: expected '('" % (path, toks[start].line))
    depth = 0
    args = [[]]
    i = start
    while i < len(toks):
        t = toks[i]
        if t.kind == "punct" and t.text in "([{":
            depth += 1
            if depth > 1:
                args[-1].append(t)
        elif t.kind == "punct" and t.text in ")]}":
            depth -= 1
            if depth == 0:
                return args, i
            args[-1].append(t)
        elif t.kind == "punct" and t.text == "," and depth == 1:
            args.append([])
        else:
            args[-1].append(t)
        i += 1
    raise Failure("%s:%d: unterminated macro call" % (path, toks[start].line))


def literal_value(arg, path, line):
    """Return the joined value of a run of adjacent string literals, or None."""
    if not arg or any(t.kind != "str" for t in arg):
        return None
    wide = [t for t in arg if t.wide]
    if wide:
        raise Failure("%s:%d: wide literal %r is used as a message id; the corpus "
                      "keys are UTF-8" % (path, wide[0].line, wide[0].text))
    return "".join(t.text for t in arg)


def escape(value):
    """Encode a message id as one line: backslash, TAB, CR and LF are escaped."""
    return (value.replace("\\", "\\\\")
                 .replace("\t", "\\t")
                 .replace("\r", "\\r")
                 .replace("\n", "\\n"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", default=str(DEFAULT_REF))
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    ref = pathlib.Path(args.ref)
    src_dir = ref / "src"
    if not src_dir.is_dir():
        print("gen_msgids: no source tree at %s" % src_dir, file=sys.stderr)
        return 2

    entries = set()
    per_family = Counter()
    per_kind = Counter()
    nonliteral = Counter()
    files = sorted(p for p in src_dir.rglob("*") if p.suffix in (".cpp", ".h"))

    for path in files:
        try:
            raw = path.read_text(encoding="utf-8", errors="strict")
        except UnicodeDecodeError as exc:
            raise Failure("%s: not valid UTF-8: %s" % (path, exc))
        rel = path.relative_to(ref)
        src = strip_comments_and_directives(raw, rel)
        toks = tokenize(src, rel)
        for idx, tok in enumerate(toks):
            if tok.kind != "id" or tok.text not in FAMILIES:
                continue
            if idx + 1 >= len(toks) or toks[idx + 1].text != "(":
                continue
            ctx_index, msgid_indices = FAMILIES[tok.text]
            macro_args, _ = split_args(toks, idx + 1, rel)
            needed = 1 + max(idx2 for idx2 in msgid_indices + ([ctx_index] if ctx_index is not None else []))
            if len(macro_args) < needed:
                raise Failure("%s:%d: %s takes at least %d arguments, saw %d"
                              % (rel, tok.line, tok.text, needed, len(macro_args)))
            values = []
            missing = False
            for k in msgid_indices:
                value = literal_value(macro_args[k], rel, tok.line)
                if value is None:
                    missing = True
                    break
                values.append(value)
            if missing:
                nonliteral[tok.text] += 1
                continue
            context = None
            if ctx_index is not None:
                context = literal_value(macro_args[ctx_index], rel, tok.line)
                if context is None:
                    nonliteral[tok.text] += 1
                    continue
            per_family[tok.text] += 1
            for value in values:
                if context is not None:
                    entries.add("%s\t%s" % (escape(context), escape(value)))
                    per_kind["contextual"] += 1
                else:
                    entries.add(escape(value))
                    per_kind["bare"] += 1

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    lines = sorted(entries)
    out.write_text("".join(line + "\n" for line in lines), encoding="utf-8")

    if not args.quiet:
        print("gen_msgids: scanned %d files under %s" % (len(files), src_dir))
        print("gen_msgids: %d distinct message ids (%d bare, %d contextual) -> %s"
              % (len(lines), per_kind["bare"], per_kind["contextual"], out))
        print("gen_msgids: calls extracted per macro family:")
        for name in sorted(FAMILIES):
            print("  %-26s %6d" % (name, per_family[name]))
        print("gen_msgids: calls skipped (non-literal argument, invisible to xgettext):")
        for name in sorted(FAMILIES):
            print("  %-26s %6d" % (name, nonliteral[name]))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Failure as exc:
        print("gen_msgids: FAILED: %s" % exc, file=sys.stderr)
        sys.exit(1)
