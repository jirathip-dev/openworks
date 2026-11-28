use std::fs;
use std::path::PathBuf;

use openworks_server::web;

fn bundle(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("index.html"), "<main></main>").unwrap();
    fs::write(root.join("assets/app.js"), "go()").unwrap();
    root
}

#[test]
fn a_bundle_file_is_served_with_its_type() {
    let root = bundle("web-file");

    let answer = web::file(&root, "/assets/app.js");

    assert_eq!(
        (answer.status, answer.kind, answer.body),
        (200, "text/javascript; charset=utf-8", b"go()".to_vec())
    );
}

#[test]
fn a_screen_path_is_served_the_index() {
    let root = bundle("web-screen");

    let answer = web::file(&root, "/projects/p1");

    assert_eq!((answer.status, answer.body), (200, b"<main></main>".to_vec()));
    assert_eq!(answer.kind, "text/html; charset=utf-8");
}

#[test]
fn a_path_out_of_the_bundle_is_refused() {
    let root = bundle("web-escape");
    fs::write(root.parent().unwrap().join("outside.txt"), "no").unwrap();

    let answer = web::file(&root, "/../outside.txt");

    assert_eq!((answer.status, answer.body), (404, b"no such file".to_vec()));
}
