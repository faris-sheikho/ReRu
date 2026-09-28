use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub struct Server {
    child: Child,
    pub port: u16,
}

impl Server {
    pub fn start() -> Self {
        Self::start_with(&[])
    }

    pub fn start_with(extra_args: &[&str]) -> Self {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let child = Command::new(env!("CARGO_BIN_EXE_reru"))
            .args(["--port", &port.to_string()])
            .args(extra_args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let server = Self { child, port };
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if server.connect().is_ok() {
                return server;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("server did not bind to port {port}");
    }

    pub fn connect(&self) -> std::io::Result<TcpStream> {
        let stream = TcpStream::connect(("127.0.0.1", self.port))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        Ok(stream)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
