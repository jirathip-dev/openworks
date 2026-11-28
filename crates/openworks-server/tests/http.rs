use openworks_server::http::{self, Answer};

#[test]
fn a_request_is_read_with_its_path_headers_and_body() {
    let raw = b"POST /clients/save?x=1 HTTP/1.1\r\nAuthorization: Bearer abc\r\nIdempotency-Key: k1\r\nContent-Length: 5\r\n\r\nhello";

    let asked = http::read(&raw[..]).unwrap();

    assert_eq!(asked.method, "POST");
    assert_eq!(asked.path, "/clients/save");
    assert_eq!(asked.bearer(), "abc");
    assert_eq!(asked.header("idempotency-key"), Some("k1"));
    assert_eq!(asked.body, b"hello");
}

#[test]
fn a_chunked_body_a_short_body_or_a_huge_body_is_refused() {
    let chunked = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
    let short = b"POST / HTTP/1.1\r\nContent-Length: 9\r\n\r\nhello";
    let huge = b"POST / HTTP/1.1\r\nContent-Length: 999999999999\r\n\r\n";

    assert_eq!(http::read(&chunked[..]).unwrap_err().status, 411);
    assert_eq!(http::read(&short[..]).unwrap_err().status, 400);
    assert_eq!(http::read(&huge[..]).unwrap_err().status, 413);
}

#[test]
fn an_answer_is_written_with_its_status_type_and_length() {
    let mut written = Vec::new();

    http::write(&mut written, &Answer::json(202, r#"{"proposal":"p1"}"#.to_owned())).unwrap();

    assert_eq!(
        String::from_utf8(written).unwrap(),
        "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: 17\r\nConnection: close\r\n\r\n{\"proposal\":\"p1\"}"
    );
}
