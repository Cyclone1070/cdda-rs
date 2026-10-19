//! Comments, as the reference's lexer handles them.

use cdda_json::parse;

#[test]
fn comments_are_skipped_wherever_whitespace_is() {
    // `Parser::Next` skips `//` to the end of the line and `/* ... */` wherever
    // whitespace would be skipped (src/third-party/flatbuffers/idl_parser.cpp:481-502).
    let plain = parse(b"{\"a\":1,\"b\":[2,3]}");
    assert_eq!(parse(b"// leading\n{\"a\":1,\"b\":[2,3]}"), plain);
    assert_eq!(parse(b"{\"a\":1,\"b\":[2,3]} // trailing"), plain);
    assert_eq!(parse(b"{\"a\":1, // mid\n \"b\":[2,3]}"), plain);
    assert_eq!(
        parse(b"/* block */ {\"a\":/* inline */1,\"b\":[/* x */2,3]/* y */}"),
        plain
    );
}

#[test]
fn a_documentation_comment_must_stand_on_its_own_line() {
    // `///` is a documentation comment and the reference insists it starts a
    // line; a block comment that never closes is also an error
    // (idl_parser.cpp:485-499).
    assert_eq!(
        parse(b"{\"a\":1} /// doc\n").err().map(|e| e.message),
        Some("a documentation comment should be on a line on its own".to_string())
    );
    assert_eq!(
        parse(b"/* never closed").err().map(|e| e.message),
        Some("end of file in comment".to_string())
    );
    // On its own line, a documentation comment is fine.
    assert!(parse(b"/// doc\n{}").is_ok());
    assert!(parse(b"{\"a\":1}\n/// doc\n").is_ok());
}
