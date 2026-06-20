use httplite::{Httplite, Request, ResponseWriter};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

struct TestServer {
    addr: SocketAddr,
    should_shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<std::io::Result<()>>>,
}

impl TestServer {
    fn start(configure: impl FnOnce(&Httplite)) -> Self {
        let app = Httplite::new("127.0.0.1:0");
        configure(&app);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let should_shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_signal = Arc::clone(&should_shutdown);

        let handle = thread::spawn(move || {
            app.serve_listener_until(listener, || shutdown_signal.load(Ordering::SeqCst))
        });

        Self {
            addr,
            should_shutdown,
            handle: Some(handle),
        }
    }

    fn get(&self, path: &str) -> HttpResponse {
        let mut stream = TcpStream::connect(self.addr).unwrap();
        let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");

        stream.write_all(request.as_bytes()).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();

        let mut raw_response = String::new();
        stream.read_to_string(&mut raw_response).unwrap();

        HttpResponse::parse(raw_response)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.should_shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);

        if let Some(handle) = self.handle.take() {
            handle.join().unwrap().unwrap();
        }
    }
}

struct HttpResponse {
    status_line: String,
    headers: HashMap<String, String>,
    body: String,
}

impl HttpResponse {
    fn parse(raw_response: String) -> Self {
        let (head, body) = raw_response.split_once("\r\n\r\n").unwrap();
        let mut lines = head.lines();
        let status_line = lines.next().unwrap().to_string();
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(": ").unwrap();
                (name.to_string(), value.to_string())
            })
            .collect();

        Self {
            status_line,
            headers,
            body: body.to_string(),
        }
    }
}

#[test]
fn routes_http_requests_to_matching_handlers() {
    fn hello(mut writer: ResponseWriter, request: Request) {
        let name = request.url().trim_start_matches("/hello/");
        writer.print_text(&format!("Hello, {name}")).unwrap();
    }

    let server = TestServer::start(|app| {
        app.add_route("/hello", hello);
    });

    let response = server.get("/hello/httplite");

    assert_eq!(response.status_line, "HTTP/1.1 200 OK");
    assert_eq!(response.headers["Content-Type"], "text/plain");
    assert_eq!(response.headers["Content-Length"], "15");
    assert_eq!(response.headers["Connection"], "close");
    assert_eq!(response.body, "Hello, httplite");
}

#[test]
fn returns_json_responses() {
    fn json(mut writer: ResponseWriter, _request: Request) {
        let mut values = HashMap::new();
        values.insert("project", "httplite");

        writer.print_hashmap_to_json(&values).unwrap();
    }

    let server = TestServer::start(|app| {
        app.add_route("/json", json);
    });

    let response = server.get("/json");

    assert_eq!(response.status_line, "HTTP/1.1 200 OK");
    assert_eq!(response.headers["Content-Type"], "application/json");
    assert_eq!(response.headers["Content-Length"], "22");
    assert_eq!(response.body, r#"{"project":"httplite"}"#);
}

#[test]
fn returns_not_found_for_unmatched_paths() {
    let server = TestServer::start(|_app| {});

    let response = server.get("/missing");

    assert_eq!(response.status_line, "HTTP/1.1 404 Not Found");
    assert_eq!(response.headers["Content-Type"], "text/plain");
    assert_eq!(response.body, "404 Not Found");
}

#[test]
fn prefers_the_longest_matching_route() {
    fn root(mut writer: ResponseWriter, _request: Request) {
        writer.print_text("root").unwrap();
    }

    fn hello(mut writer: ResponseWriter, _request: Request) {
        writer.print_text("hello").unwrap();
    }

    let server = TestServer::start(|app| {
        app.add_route("/", root);
        app.add_route("/hello", hello);
    });

    let response = server.get("/hello/world");

    assert_eq!(response.status_line, "HTTP/1.1 200 OK");
    assert_eq!(response.body, "hello");
}
