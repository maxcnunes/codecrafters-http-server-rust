// Basic HTTP implementation.
//
// HTTP/1.1 RFC - https://datatracker.ietf.org/doc/html/rfc2616/
//
// Other helpful references:
// - https://thepacketgeek.com/rust/tcpstream/reading-and-writing/

use std::env;
use std::fs;
use std::io;
use std::io::BufReader;
use std::io::{BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

fn main() {
    let mut args = env::args();
    println!("Args {:?}", args);

    // Parse CLI args
    let mut param_dir: Option<String> = None;
    while let Some(arg) = args.next() {
        println!("arg={:?}", arg);
        if arg == "--directory" {
            if let Some(d) = args.next() {
                param_dir = Some(d);
            }
        }
    }
    println!("Dir {:?}", param_dir);

    // Creates an ARC (Atomically Reference Counted) to share this immutable value
    // across multiple threads.
    let dir = Arc::new(param_dir);

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Here there is no value specification as it is a pointer to a
                // reference in the memory heap.
                // This creates another pointer to the same allocation, increasing the
                // strong reference count.
                let dir = Arc::clone(&dir);

                //  Handle connection in a thread so this server
                // can handle multiple concurrent connections.
                thread::spawn(move || {
                    println!("accepted new connection ({})", stream.peer_addr().unwrap());
                    handle_connection(stream, dir);
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

enum Status {
    // 2xx
    OK,
    Created,

    // 4xx
    NotFound,

    // 5xx
    InternalServerError,
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    http_info: String,
    // Use vector instead of a hash map because
    // header keys are not unique and could there be multiple
    // headers for the same key.
    headers: Vec<(String, String)>,
    body: String,
}

impl Request {
    fn get_header(&self, key: &str) -> Option<String> {
        for (k, v) in self.headers.iter() {
            if key == k {
                return Some(v.to_string());
            }
        }

        return None;
    }
}

// TODO: handle errors properly
fn handle_connection(mut stream: TcpStream, dir: Arc<Option<String>>) {
    // NOTE: We must read the data before writing any response,
    // otherwise the stream will automatically close the connection
    // and return "Recv failure: Connection reset by peer" to the client.

    // Wrap stream with Bufreader
    let mut reader = BufReader::new(&mut stream);

    let mut req = Request {
        method: String::new(),
        path: String::new(),
        http_info: String::new(),
        headers: vec![],
        body: String::new(),
    };

    let mut has_body = false;
    let mut is_first = true;

    // Read all data from this stream
    let mut buf: Vec<u8> = Vec::new();

    loop {
        let _ = buf.clear();
        let bytes = reader.read_until(b'\n', &mut buf).unwrap();
        if bytes == 0 {
            break;
        };

        let line = std::str::from_utf8(&buf).unwrap();

        println!("line {:?}", line);
        if line == "\r\n" {
            // This means the whole header has been read,
            // and any data next is part of the body.
            break;
        }

        let line = line.strip_suffix("\r\n").unwrap();

        // parse request info
        if is_first {
            is_first = false;
            let parts: Vec<&str> = line.split(" ").collect();
            if parts.len() != 3 {
                panic!("Bad first line format {:?}", parts);
            }

            req.method = parts[0].to_string();
            req.path = parts[1].to_string();
            req.http_info = parts[2].to_string();
            continue;
        }

        // parse headers
        if let Some(parts) = line.split_once(": ") {
            let key = parts.0.to_string();
            let val = parts.1.to_string();

            if key == "Content-Length" {
                has_body = true;
            }

            req.headers.push((key, val));

            continue;
        }
    }

    if has_body {
        let received: Vec<u8> = reader.fill_buf().unwrap().to_vec();
        reader.consume(received.len());
        req.body = String::from_utf8(received).unwrap();
    }

    println!("Request {:?}", req);

    println!("Responding");

    // TODO: Could use enum?
    let mut res_content_type: Option<&str> = None;
    let mut res_body: Option<Vec<u8>> = None;

    let mut status: Status = Status::NotFound;

    // Handle routes
    if req.method == "GET" && req.path == "/" {
        status = Status::OK;
    } else if req.method == "GET" && req.path.starts_with("/echo/") {
        let parts: Vec<&str> = req.path.split("/").skip(2).collect();
        println!("Parts {:?}", parts);
        let param = parts.join("/");
        println!("Param {}", param);
        res_body = Some(param.to_string().into_bytes());
        res_content_type = Some("text/plain");
        status = Status::OK;
    } else if req.method == "GET" && req.path == "/user-agent" {
        res_body = Some(req.get_header("User-Agent").unwrap().into_bytes());
        res_content_type = Some("text/plain");
        status = Status::OK;
    } else if req.method == "GET" && req.path.starts_with("/files/") {
        let parts: Vec<&str> = req.path.split("/").skip(2).collect();
        println!("Parts {:?}", parts);
        let filename = parts[0];
        println!("File name {}", filename);
        let filepath = dir.as_ref().to_owned().unwrap() + filename;
        println!("File path {}", filepath);

        match fs::read(&filepath) {
            Ok(binary) => {
                res_body = Some(binary);
                res_content_type = Some("application/octet-stream");
                status = Status::OK;
            }
            Err(ref e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    status = Status::NotFound;
                } else {
                    println!("Unexpected error reading file: {}, err {}", filepath, e);
                    status = Status::InternalServerError;
                }
            }
        }
    } else if req.method == "POST" && req.path.starts_with("/files/") {
        let parts: Vec<&str> = req.path.split("/").skip(2).collect();
        println!("Parts {:?}", parts);
        let filename = parts[0];
        println!("File name {}", filename);
        let filepath = dir.as_ref().to_owned().unwrap() + filename;
        println!("File path {}", filepath);

        fs::write(filepath, req.body).unwrap();
        res_content_type = Some("application/octet-stream");
        status = Status::Created;
    }

    // Write response:
    //
    // Respond with "HTTP/1.1 200 OK\r\n\r\n" (there are two \r\ns at the end)
    //
    // * "HTTP/1.1 200 OK" is the HTTP Status Line.
    // * "\r\n", also known as CRLF, is the end-of-line marker that HTTP uses.
    // * The first "\r\n" signifies the end of the status line.
    // * The second "\r\n" signifies the end of the response headers section (which is empty in this case).
    //
    let status_text = match status {
        Status::OK => "200 OK",
        Status::Created => "201 Created",
        Status::NotFound => "404 Not Found",
        Status::InternalServerError => "500 Internal Server Error",
    };

    write!(&mut stream, "HTTP/1.1 {}\r\n", status_text).unwrap();

    if let Some(body) = &res_body {
        write!(&mut stream, "Content-Type: {}\n", res_content_type.unwrap()).unwrap();
        write!(&mut stream, "Content-Length: {}\n", body.len()).unwrap();
    } else {
        write!(&mut stream, "\r\n").unwrap();
    }

    write!(&mut stream, "\r\n").unwrap();

    if let Some(body) = &res_body {
        stream.write(&body).unwrap();
    }

    stream.flush().unwrap();
    println!("Done");
}
