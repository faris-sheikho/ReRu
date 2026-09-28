mod common;

#[test]
fn binds_to_a_port() {
    let server = common::Server::start();
    assert!(server.connect().is_ok());
}
