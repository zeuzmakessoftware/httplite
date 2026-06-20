use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub type Handler = fn(ResponseWriter, Request);

#[derive(Clone)]
struct Route {
    path: String,
    handler: Handler,
}

pub struct Httplite {
    port: String,
    routes: Arc<Mutex<Vec<Route>>>,
}

impl Httplite {
    pub fn new(port: &str) -> Httplite {
        Httplite {
            port: port.to_string(),
            routes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_route(&self, route: &str, handler: Handler) {
        let mut routes = self.routes.lock().unwrap();
        if let Some(existing_route) = routes.iter_mut().find(|item| item.path == route) {
            existing_route.handler = handler;
            return;
        }

        routes.push(Route {
            path: route.to_string(),
            handler,
        });
    }

    pub fn listen(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.addr())?;
        self.serve_listener(listener)
    }

    pub fn serve_listener(&self, listener: TcpListener) -> std::io::Result<()> {
        for stream in listener.incoming() {
            self.handle_connection(stream?)?;
        }

        Ok(())
    }

    pub fn serve_listener_until<F>(
        &self,
        listener: TcpListener,
        should_shutdown: F,
    ) -> std::io::Result<()>
    where
        F: Fn() -> bool,
    {
        listener.set_nonblocking(true)?;

        while !should_shutdown() {
            match listener.accept() {
                Ok((stream, _addr)) => self.handle_connection(stream)?,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }

        Ok(())
    }

    fn addr(&self) -> String {
        let addr = if self.port.starts_with(':') {
            format!("127.0.0.1{}", self.port)
        } else {
            self.port.clone()
        };

        addr
    }

    fn handle_connection(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut buffer = [0; 8192];
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            return Ok(());
        }

        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
        let request = Request::new(request_str.to_string());
        let url = request.url().to_string();

        if let Some(handler) = self.handler_for(&url) {
            handler(ResponseWriter::new(stream), request);
        } else {
            let mut response_writer = ResponseWriter::new(stream);
            response_writer.print_text_with_status(StatusCode::NotFound, "404 Not Found")?;
        }

        Ok(())
    }

    fn handler_for(&self, url: &str) -> Option<Handler> {
        let routes = self.routes.lock().unwrap();
        routes
            .iter()
            .filter(|route| url.starts_with(&route.path))
            .max_by_key(|route| route.path.len())
            .map(|route| route.handler)
    }
}

pub struct Request {
    raw: String,
}

impl Request {
    pub fn new(raw: String) -> Self {
        Self { raw }
    }

    pub fn url(&self) -> &str {
        let request_line = self.raw.lines().next().unwrap_or_default();
        request_line.split_whitespace().nth(1).unwrap_or("/")
    }

    pub fn method(&self) -> &str {
        let request_line = self.raw.lines().next().unwrap_or_default();
        request_line.split_whitespace().next().unwrap_or("GET")
    }
}

pub struct ResponseWriter {
    stream: TcpStream,
}

impl ResponseWriter {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn write(&mut self, response: &str) -> std::io::Result<()> {
        self.stream.write_all(response.as_bytes())?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn print_text(&mut self, text: &str) -> std::io::Result<()> {
        self.print_text_with_status(StatusCode::Ok, text)
    }

    pub fn print_text_with_status(
        &mut self,
        status: StatusCode,
        text: &str,
    ) -> std::io::Result<()> {
        let response = format!(
            "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status.reason_phrase(),
            text.len(),
            text,
        );
        self.write(&response)
    }

    pub fn print_hashmap_to_json<K, V>(&mut self, hashmap: &HashMap<K, V>) -> std::io::Result<()>
    where
        K: ToString,
        V: ToJson,
    {
        let json = hashmap.to_json();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            json.len(),
            json,
        );
        self.write(&response)
    }
}

pub enum StatusCode {
    Ok,
    NotFound,
}

impl StatusCode {
    fn reason_phrase(&self) -> &'static str {
        match self {
            Self::Ok => "200 OK",
            Self::NotFound => "404 Not Found",
        }
    }
}

pub trait ToJson {
    fn to_json(&self) -> String;
}

impl ToJson for String {
    fn to_json(&self) -> String {
        json_string(self)
    }
}

impl ToJson for &str {
    fn to_json(&self) -> String {
        json_string(self)
    }
}

impl ToJson for i32 {
    fn to_json(&self) -> String {
        self.to_string()
    }
}

impl ToJson for bool {
    fn to_json(&self) -> String {
        self.to_string()
    }
}

impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self) -> String {
        json_array(self.iter())
    }
}

impl<T: ToJson, const N: usize> ToJson for [T; N] {
    fn to_json(&self) -> String {
        json_array(self.iter())
    }
}

impl<K: ToString, V: ToJson> ToJson for HashMap<K, V> {
    fn to_json(&self) -> String {
        let fields = self
            .iter()
            .map(|(key, value)| format!("{}:{}", json_string(&key.to_string()), value.to_json()))
            .collect::<Vec<_>>()
            .join(",");

        format!("{{{fields}}}")
    }
}

fn json_array<'a, T, I>(items: I) -> String
where
    T: ToJson + 'a,
    I: Iterator<Item = &'a T>,
{
    let values = items.map(ToJson::to_json).collect::<Vec<_>>().join(",");
    format!("[{values}]")
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');

    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }

    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::{Request, ToJson};
    use std::collections::HashMap;

    #[test]
    fn request_method_and_url_default_safely() {
        let request = Request::new(String::new());

        assert_eq!(request.method(), "GET");
        assert_eq!(request.url(), "/");
    }

    #[test]
    fn request_method_and_url_parse_the_request_line() {
        let request = Request::new("POST /things/42 HTTP/1.1\r\nHost: example.com\r\n\r\n".into());

        assert_eq!(request.method(), "POST");
        assert_eq!(request.url(), "/things/42");
    }

    #[test]
    fn json_helpers_handle_empty_collections() {
        let empty_vec: Vec<String> = Vec::new();
        let empty_map: HashMap<String, String> = HashMap::new();

        assert_eq!(empty_vec.to_json(), "[]");
        assert_eq!(empty_map.to_json(), "{}");
    }

    #[test]
    fn json_helpers_escape_strings() {
        assert_eq!("hello \"rust\"\n".to_json(), "\"hello \\\"rust\\\"\\n\"");
    }
}
