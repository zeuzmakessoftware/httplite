use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

pub struct Httplite {
    port: String,
    routes: Arc<Mutex<HashMap<String, fn(ResponseWriter, Request)>>>,
}

impl Httplite {
    pub fn new(port: &str) -> Httplite {
        Httplite {
            port: port.to_string(),
            routes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_route(&self, route: &str, handler: fn(ResponseWriter, Request)) {
        let mut routes = self.routes.lock().unwrap();
        routes.insert(route.to_string(), handler);
    }

    pub fn listen(&self) -> std::io::Result<()> {
        let addr = if self.port.starts_with(':') {
            format!("127.0.0.1{}", self.port)
        } else {
            self.port.clone()
        };

        let listener = TcpListener::bind(&addr)?;

        for stream in listener.incoming() {
            let routes = Arc::clone(&self.routes);
            let mut stream = stream?;
            let mut buffer = [0; 1024];
            stream.read(&mut buffer)?;

            let request_str = String::from_utf8_lossy(&buffer[..]);
            let request = Request::new(request_str.to_string());
            let url = request.url().to_string();
            let routes = routes.lock().unwrap();

            let mut found = false;
            {
                let response_writer = ResponseWriter::new(stream.try_clone()?);

                for (route, handler) in routes.iter() {
                    if url.starts_with(route) {
                        handler(response_writer, request);
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                let mut response_writer = ResponseWriter::new(stream);
                response_writer.print_text("404 Not Found")?;
            }
        }

        Ok(())
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
    stream: std::net::TcpStream,
}

impl ResponseWriter {
    pub fn new(stream: std::net::TcpStream) -> Self {
        Self { stream }
    }

    pub fn write(&mut self, response: &str) -> std::io::Result<()> {
        self.stream.write(response.as_bytes())?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn print_text(&mut self, text: &str) -> std::io::Result<()> {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n{}",
            text
        );
        self.write(&response)
    }
}