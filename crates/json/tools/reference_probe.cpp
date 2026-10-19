// Reference probe for `tests/reference_parity.rs`.
//
// It parses each path given on stdin exactly the way the reference loader does
// — `flatbuffers::Parser` with `strict_json` and `use_flexbuffers`, as set up in
// references/Cataclysm-DDA/src/flexbuffer_cache.cpp:59-70 — and prints
//
//     <path>\tOK\t<hash>
//     <path>\tFAIL\t<error>
//
// where `<hash>` is an FNV-1a 64 of a canonical encoding of the resulting
// flexbuffer:
//
//     null -> null          bool -> true|false      int  -> i<decimal>
//     uint -> u<decimal>    float -> f<bits:016x)   string -> s<fnv64(bytes)>
//     map -> {k<fnv64(key)>=<value>,...}            vector -> [<value>,...]
//
// Keys are hashed as the NUL-terminated C strings flexbuffers compares, and
// maps are walked in stored (sorted) order, so the encoding pins key order,
// integer-versus-float typing, escape decoding and NUL handling at once.
//
// Build with tools/build_reference_probe.sh.

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <sstream>
#include <algorithm>
#include <iterator>
#include <locale>
#include <string>
#include <string_view>
#include <vector>

#include "flatbuffers/flexbuffers.h"
#include "flatbuffers/idl.h"

static uint64_t fnv(const std::string &s) {
  uint64_t h = 1469598103934665603ULL;
  for (unsigned char c : s) {
    h ^= c;
    h *= 1099511628211ULL;
  }
  return h;
}

static void canon(const flexbuffers::Reference &r, std::string &out);

static void canon_map(const flexbuffers::Reference &r, std::string &out) {
  auto map = r.AsMap();
  auto keys = map.Keys();
  out += "{";
  for (size_t i = 0; i < keys.size(); i++) {
    if (i) out += ",";
    std::string key = keys[i].AsKey();
    char buffer[32];
    snprintf(buffer, sizeof(buffer), "k%016llx", (unsigned long long)fnv(key));
    out += buffer;
    out += "=";
    canon(map.Values()[i], out);
  }
  out += "}";
}

static void canon(const flexbuffers::Reference &r, std::string &out) {
  char buffer[64];
  switch (r.GetType()) {
    case flexbuffers::FBT_NULL:
      out += "null";
      break;
    case flexbuffers::FBT_BOOL:
      out += r.AsBool() ? "true" : "false";
      break;
    case flexbuffers::FBT_INT:
      snprintf(buffer, sizeof(buffer), "i%lld", (long long)r.AsInt64());
      out += buffer;
      break;
    case flexbuffers::FBT_UINT:
      snprintf(buffer, sizeof(buffer), "u%llu", (unsigned long long)r.AsUInt64());
      out += buffer;
      break;
    case flexbuffers::FBT_FLOAT: {
      double value = r.AsDouble();
      uint64_t bits;
      std::memcpy(&bits, &value, sizeof(bits));
      snprintf(buffer, sizeof(buffer), "f%016llx", (unsigned long long)bits);
      out += buffer;
      break;
    }
    case flexbuffers::FBT_STRING: {
      auto text = r.AsString();
      snprintf(buffer, sizeof(buffer), "s%016llx",
               (unsigned long long)fnv(std::string(text.c_str(), text.length())));
      out += buffer;
      break;
    }
    case flexbuffers::FBT_MAP:
      canon_map(r, out);
      break;
    case flexbuffers::FBT_VECTOR: {
      auto vector = r.AsVector();
      out += "[";
      for (size_t i = 0; i < vector.size(); i++) {
        if (i) out += ",";
        canon(vector[i], out);
      }
      out += "]";
      break;
    }
    default:
      out += "?";
      break;
  }
}


// ---------------------------------------------------------------------------
// A verbatim copy of `JsonOut`'s layout logic, from
// references/Cataclysm-DDA/src/json.cpp:2226-2440 (metafunctions removed: only
// containers, null, booleans, integers, doubles and strings are written). The
// only change is the stream: an ostringstream, so the probe can compare texts.
// ---------------------------------------------------------------------------

namespace json_out {

class Out {
 private:
  std::ostringstream stream;
  bool pretty_print;
  std::vector<bool> need_wrap;
  int indent_level = 0;
  bool need_separator = false;

 public:
  explicit Out(bool pretty) : pretty_print(pretty) {
    // Ensure consistent and locale-independent formatting of numerals
    // (json.cpp:2230-2236).
    stream.imbue(std::locale::classic());
    stream.setf(std::ios_base::showpoint);
    stream.setf(std::ios_base::dec, std::ostream::basefield);
    stream.setf(std::ios_base::fixed, std::ostream::floatfield);
    stream.setf(std::ios_base::boolalpha);
  }

  std::string str() const { return stream.str(); }

  void write_indent() {
    std::fill_n(std::ostream_iterator<char>(stream), indent_level * 2, ' ');
  }

  void write_separator() {
    if (!need_separator) return;
    stream.put(',');
    if (pretty_print) {
      if (indent_level < 2 || need_wrap.back()) {
        stream.put('\n');
        write_indent();
      } else {
        stream.put(' ');
      }
    }
    need_separator = false;
  }

  void write_member_separator() {
    if (pretty_print) {
      stream.write(": ", 2);
    } else {
      stream.put(':');
    }
    need_separator = false;
  }

  void start_pretty() {
    if (pretty_print) {
      indent_level += 1;
      if (indent_level < 2 || need_wrap.back()) {
        stream.put('\n');
        write_indent();
      } else {
        stream.put(' ');
      }
    }
  }

  void end_pretty() {
    if (pretty_print) {
      indent_level -= 1;
      if (indent_level < 1 || need_wrap.back()) {
        stream.put('\n');
        write_indent();
      } else {
        stream.put(' ');
      }
    }
  }

  void start_object(bool wrap) {
    if (need_separator) write_separator();
    stream.put('{');
    need_wrap.push_back(wrap);
    start_pretty();
    need_separator = false;
  }

  void end_object() {
    end_pretty();
    need_wrap.pop_back();
    stream.put('}');
    need_separator = true;
  }

  void start_array(bool wrap) {
    if (need_separator) write_separator();
    stream.put('[');
    need_wrap.push_back(wrap);
    start_pretty();
    need_separator = false;
  }

  void end_array() {
    end_pretty();
    need_wrap.pop_back();
    stream.put(']');
    need_separator = true;
  }

  void write_null() {
    if (need_separator) write_separator();
    stream.write("null", 4);
    need_separator = true;
  }

  void write(bool value) {
    if (need_separator) write_separator();
    stream << value;
    need_separator = true;
  }

  void write(long long value) {
    if (need_separator) write_separator();
    stream << value;
    need_separator = true;
  }

  void write(unsigned long long value) {
    if (need_separator) write_separator();
    stream << value;
    need_separator = true;
  }

  void write(double value) {
    if (need_separator) write_separator();
    stream << value;
    need_separator = true;
  }

  void write(std::string_view value) {
    if (need_separator) write_separator();
    stream.put('"');
    for (const char &i : value) {
      unsigned char ch = i;
      if (ch == '"') {
        stream.write("\\\"", 2);
      } else if (ch == '\\') {
        stream.write("\\\\", 2);
      } else if (ch == '/') {
        stream.put('/');
      } else if (ch == '\b') {
        stream.write("\\b", 2);
      } else if (ch == '\f') {
        stream.write("\\f", 2);
      } else if (ch == '\n') {
        stream.write("\\n", 2);
      } else if (ch == '\r') {
        stream.write("\\r", 2);
      } else if (ch == '\t') {
        stream.write("\\t", 2);
      } else if (ch < 0x20) {
        stream.write("\\u00", 4);
        stream.put((ch < 0x10) ? '0' : '1');
        char remainder = ch & 0x0F;
        if (remainder < 0x0A) {
          stream.put('0' + remainder);
        } else {
          stream.put('A' + (remainder - 0x0A));
        }
      } else {
        stream.put(ch);
      }
    }
    stream.put('"');
    need_separator = true;
  }

  void member(std::string_view name) {
    write(name);
    write_member_separator();
  }
};

// Walks a parsed flexbuffer through the writer above. Containers use
// `wrap = false`, the reference's default argument (json.h:754, json.h:760).
void write_value(const flexbuffers::Reference &r, Out &out) {
  switch (r.GetType()) {
    case flexbuffers::FBT_NULL:
      out.write_null();
      break;
    case flexbuffers::FBT_BOOL:
      out.write(static_cast<bool>(r.AsBool()));
      break;
    case flexbuffers::FBT_INT:
      out.write(static_cast<long long>(r.AsInt64()));
      break;
    case flexbuffers::FBT_UINT:
      out.write(static_cast<unsigned long long>(r.AsUInt64()));
      break;
    case flexbuffers::FBT_FLOAT:
      out.write(r.AsDouble());
      break;
    case flexbuffers::FBT_STRING: {
      auto text = r.AsString();
      out.write(std::string_view(text.c_str(), text.length()));
      break;
    }
    case flexbuffers::FBT_MAP: {
      auto map = r.AsMap();
      auto keys = map.Keys();
      out.start_object(false);
      for (size_t i = 0; i < keys.size(); i++) {
        out.member(std::string(keys[i].AsKey()));
        write_value(map.Values()[i], out);
      }
      out.end_object();
      break;
    }
    case flexbuffers::FBT_VECTOR: {
      auto vector = r.AsVector();
      out.start_array(false);
      for (size_t i = 0; i < vector.size(); i++) {
        write_value(vector[i], out);
      }
      out.end_array();
      break;
    }
    default:
      out.write_null();
      break;
  }
}

}  // namespace json_out

int main(int argc, char **argv) {
  // `writer` mode: print "<path>\0<compact>\0<pretty>\0" per input path, so
  // the Rust test can compare its own writer's bytes with the reference's.
  const bool writer_mode = argc > 1 && std::string(argv[1]) == "writer";
  std::string path;
  while (std::getline(std::cin, path)) {
    if (path.empty()) continue;
    std::ifstream in(path, std::ios::binary);
    std::ostringstream contents;
    contents << in.rdbuf();
    std::string buffer = contents.str();

    flatbuffers::IDLOptions options;
    options.strict_json = true;
    options.use_flexbuffers = true;
    options.no_warnings = true;
    flatbuffers::Parser parser{options};
    flexbuffers::Builder builder;
    if (!parser.ParseFlexBuffer(buffer.c_str(), path.c_str(), &builder)) {
      std::cout << path << "\tFAIL\t" << parser.error_ << "\n";
      continue;
    }
    auto root = flexbuffers::GetRoot(builder.GetBuffer().data(), builder.GetBuffer().size());
    if (writer_mode) {
      json_out::Out compact(false);
      write_value(root, compact);
      json_out::Out pretty(true);
      write_value(root, pretty);
      std::cout << path << '\0' << compact.str() << '\0' << pretty.str() << '\0';
      continue;
    }
    std::string encoded;
    canon(root, encoded);
    char digest[32];
    snprintf(digest, sizeof(digest), "%016llx", (unsigned long long)fnv(encoded));
    std::cout << path << "\tOK\t" << digest << "\n";
  }
  return 0;
}
