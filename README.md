# httplite
A super lightweight HTTP server written in Rust, made to resemble the functionality of the "NET/HTTP" module in Go. Still in early access and working on docs.

# Installing httplite
Include this is your Cargo.toml in your project.
`
[dependencies]
httplite = "0.1.1"
`

# Basic examples
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

Hosting JSON example:
```
use httplite::{Httplite, ResponseWriter, Request};
use std::collections::HashMap;

fn main() {
    println!("Server is running at http://localhost:8080");
    let port = Httplite::new(":8080");
    port.add_route("/json", json_server);
    port.listen().unwrap();
}

fn json_server(mut w: ResponseWriter, r: Request) {
    let mut map = HashMap::new();
    map.insert("JSON Thing".to_string(), "This is a thing");
    map.insert("Another JSON Thing".to_string(), "This is another thing");
    w.print_hashmap_to_json(&map).unwrap();
}
```

# Latest Notable Updates
0.1.1
Added the ability to host hashmaps as serialized json on an endpoint.