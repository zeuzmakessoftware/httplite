# httplite
A super lightweight HTTP server written in Rust, made to resemble the functionality of the "NET/HTTP" module in Go.

# Installing httplite
Include this is your Cargo.toml in your project.
`
[dependencies]
httplite = "0.1.0"
`

# Basic example
Hello world example:
```
use httplite::{Httplite, ResponseWriter, Request};

fn main() {
    println!("Server is running at http://localhost:8080");
    let port = Httplite::new(":8080");
    port.add_route("/hello", hello_server);
    port.listen().unwrap();
}

fn hello_server(mut w: ResponseWriter, r: Request) {
    let response_text = format!("Hello, {}", r.url().trim_start_matches("/hello/"));
    w.print_text(&response_text).unwrap();
}
```