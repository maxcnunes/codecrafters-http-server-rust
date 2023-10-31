// Basic HTTP implementation.
//
// HTTP/1.1 RFC - https://datatracker.ietf.org/doc/html/rfc2616/
//
// Other helpful references:
// - https://developer.mozilla.org/en-US/docs/Web/HTTP
// - https://thepacketgeek.com/rust/tcpstream/reading-and-writing/

use std::env;
use std::fs;
use std::io;
use std::io::BufReader;
use std::io::{BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::thread;

fn main() {
    let mut args = env::args();

    // Parse CLI args
    //  * --directory {string}
    let mut param_dir: Option<String> = None;
    while let Some(arg) = args.next() {
        if arg == "--directory" {
            if let Some(d) = args.next() {
                param_dir = Some(d);
            }
        }
    }

    // Creates an ARC (Atomically Reference Counted) to share this immutable value
    // across multiple threads.
    let dir = Arc::new(param_dir);

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    println!("Running server at 127.0.0.1:4221");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Here there is no value specification as it is a pointer to a
                // reference in the memory heap.
                // This creates another pointer to the same allocation, increasing the
                // strong reference count.
                // NOTE: We could probably just clone "dir" since it is just
                // a string, but I will keep the ARC usage as reference
                // of how to support sharing data across multiple threads.
                let dir = Arc::clone(&dir);

                // Handle connection in a thread so this server
                // can handle multiple concurrent connections.
                thread::spawn(move || {
                    println!("Accepted new connection ({})", stream.peer_addr().unwrap());
                    if let Err(err) = handle_connection(stream, dir) {
                        // TODO: Should we shutdown the connection on errors?
                        println!("Error: {:?}", err);
                    }
                });
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}

enum Status {
    // 2xx
    OK,      // 200
    Created, // 201

    // 4xx
    NotFound, // 404

    // 5xx
    InternalServerError, // 500
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

#[derive(Debug)]
enum Error {
    Request(String),
    Response(String),
}

struct Response {
    status: Status,
    body: Option<Vec<u8>>,
    content_type: Option<String>,
}

fn handle_connection(stream: TcpStream, dir: Arc<Option<String>>) -> Result<(), Error> {
    // NOTE: We must read the data before writing any response,
    // otherwise the stream will automatically close the connection
    // and return "Recv failure: Connection reset by peer" to the client.

    let req = read_request(&stream)?;

    // Handle routes
    let res = match req.method.as_str() {
        "GET" if req.path == "/" => handle_get_root(&req)?,
        "GET" if req.path.starts_with("/echo/") => handle_get_echo(&req)?,
        "GET" if req.path == "/user-agent" => handle_get_user_agent(&req)?,
        "GET" if req.path.starts_with("/files/") => handle_get_file(&req, dir)?,
        "POST" if req.path.starts_with("/files/") => handle_post_file(&req, dir)?,
        _ => Response {
            status: Status::NotFound,
            body: None,
            content_type: None,
        },
    };

    write_response(&stream, &res)?;

    println!("Request completed");
    Ok(())
}

fn read_request(mut stream: &TcpStream) -> Result<Request, Error> {
    // Wrap stream with Bufreader
    let mut reader = BufReader::new(&mut stream);

    let mut req = Request {
        method: String::new(),
        path: String::new(),
        http_info: String::new(),
        headers: vec![],
        body: String::new(),
    };

    let mut is_first_line = true;
    let mut has_body = false;

    // Read request data
    //
    // A request message from a client to a server includes, within the
    // first line of that message, the method to be applied to the resource,
    // the identifier of the resource, and the protocol version in use.
    //
    //      Request       = Request-Line
    //                      *(( general-header
    //                       | request-header
    //                       | entity-header ) CRLF)
    //                      CRLF
    //                      [ message-body ]
    //
    // Reference: https://datatracker.ietf.org/doc/html/rfc2616/#section-5
    loop {
        let mut buf: Vec<u8> = Vec::new();
        // Read each request-line one by one.
        let bytes = reader
            .read_until(b'\n', &mut buf)
            .map_err(|e| Error::Request(format!("error reading buffer: {}", e)))?;

        if bytes == 0 {
            // It is empty, nothing else to read.
            break;
        };

        let line = std::str::from_utf8(&buf)
            .map_err(|e| Error::Request(format!("error parsing line buffer to string: {}", e)))?;

        println!("line {:?}", line);

        if line == "\r\n" {
            // This means the whole header has been read,
            // and any data next is part of the body.
            break;
        }

        let line = line
            .strip_suffix("\r\n")
            .ok_or(Error::Request("error stripping CRLF out".to_string()))?;

        // Process the general-header, which is always the first request-line.
        // Example: "GET /pub/WWW/TheProject.html HTTP/1.1".
        if is_first_line {
            is_first_line = false;

            let parts: Vec<&str> = line.split(" ").collect();
            if parts.len() != 3 {
                panic!("Bad general-header format {:?}", parts);
            }

            req.method = parts[0].to_string();
            req.path = parts[1].to_string();
            req.http_info = parts[2].to_string();
            continue;
        }

        // Process request-headers
        if let Some(parts) = line.split_once(": ") {
            let key = parts.0.to_string();
            let val = parts.1.to_string();

            if key == "Content-Length" {
                // If Content-Length header is present it means there should
                // be a message-body at the end of the request-message.
                has_body = true;
            }

            req.headers.push((key, val));
            continue;
        }
    }

    // Read the message-body out of the previous loop because the message-body
    // might not end with a `\n` so we cannot rely on "read until \n"
    // otherwise the reader would stuck forever waiting for a `\n`.
    // Therefore, if it was detected there is a message-body, it
    // just reads the rest of the request-message as the message-body.
    if has_body {
        let received: Vec<u8> = reader
            .fill_buf()
            .map_err(|e| Error::Request(format!("error reading message-body: {}", e)))?
            .to_vec();

        reader.consume(received.len());
        req.body = String::from_utf8(received)
            .map_err(|e| Error::Request(format!("error reading message-body: {}", e)))?;
    }

    println!("Request {:?}", req);
    Ok(req)
}

fn write_response(mut stream: &TcpStream, res: &Response) -> Result<(), Error> {
    // Write the response:
    //
    // Respond with "HTTP/1.1 200 OK\r\n\r\n" (there are two \r\ns at the end)
    //
    // * "HTTP/1.1 200 OK" is the HTTP Status Line.
    // * "\r\n", also known as CRLF, is the end-of-line marker that HTTP uses.
    // * The first "\r\n" signifies the end of the status line.
    // * The second "\r\n" signifies the end of the response headers section (which is empty in this case).
    //
    let status_text = match res.status {
        Status::OK => "200 OK",
        Status::Created => "201 Created",
        Status::NotFound => "404 Not Found",
        Status::InternalServerError => "500 Internal Server Error",
    };

    write!(&mut stream, "HTTP/1.1 {}\r\n", status_text)
        .map_err(|e| Error::Response(format!("error writing response general-header: {}", e)))?;

    match (&res.body, &res.content_type) {
        (Some(body), Some(content_type)) => {
            write!(&mut stream, "Content-Type: {}\n", content_type).map_err(|e| {
                Error::Response(format!("error writing response Content-Type header: {}", e))
            })?;

            write!(&mut stream, "Content-Length: {}\n", body.len()).map_err(|e| {
                Error::Response(format!(
                    "error writing response Content-Length header: {}",
                    e
                ))
            })?;
        }
        _ => {}
    }

    write!(&mut stream, "\r\n")
        .map_err(|e| Error::Response(format!("error writing response CRLF: {}", e)))?;

    if let Some(body) = &res.body {
        stream
            .write(&body)
            .map_err(|e| Error::Response(format!("error writing message-body: {}", e)))?;
    }

    // Flush connection stream.
    stream
        .flush()
        .map_err(|e| Error::Response(format!("error flushing connection stream: {}", e)))?;

    Ok(())
}

fn handle_get_root(_req: &Request) -> Result<Response, Error> {
    Ok(Response {
        status: Status::OK,
        body: None,
        content_type: None,
    })
}

fn handle_get_echo(req: &Request) -> Result<Response, Error> {
    let parts: Vec<&str> = req.path.split("/").skip(2).collect();
    let param = parts.join("/");

    Ok(Response {
        status: Status::OK,
        body: Some(param.to_string().into_bytes()),
        content_type: Some("text/plain".to_string()),
    })
}

fn handle_get_user_agent(req: &Request) -> Result<Response, Error> {
    Ok(Response {
        status: Status::OK,
        body: Some(
            req.get_header("User-Agent")
                .unwrap_or("".to_string())
                .into_bytes(),
        ),
        content_type: Some("text/plain".to_string()),
    })
}

fn handle_get_file(req: &Request, dir: Arc<Option<String>>) -> Result<Response, Error> {
    let parts: Vec<&str> = req.path.split("/").skip(2).collect();
    println!("Parts {:?}", parts);

    let filename = parts[0];
    println!("File name {}", filename);

    let dirpath = dir
        .as_ref()
        .to_owned()
        .ok_or(Error::Response("error getting directory path".to_string()))?;

    let filepath = Path::new(&dirpath).join(filename);
    println!("File path {:?}", filepath);

    let status: Status;
    let mut body: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    match fs::read(&filepath) {
        Ok(binary) => {
            body = Some(binary);
            content_type = Some("application/octet-stream".to_string());
            status = Status::OK;
        }
        Err(ref e) => {
            if e.kind() == io::ErrorKind::NotFound {
                status = Status::NotFound;
            } else {
                println!(
                    "Error: Unexpected error reading file: {:?}, err {}",
                    filepath, e
                );
                status = Status::InternalServerError;
            }
        }
    }

    Ok(Response {
        status,
        body,
        content_type,
    })
}

fn handle_post_file(req: &Request, dir: Arc<Option<String>>) -> Result<Response, Error> {
    let parts: Vec<&str> = req.path.split("/").skip(2).collect();
    println!("Parts {:?}", parts);

    let filename = parts[0];
    println!("File name {}", filename);

    let dirpath = dir
        .as_ref()
        .to_owned()
        .ok_or(Error::Response("error getting directory path".to_string()))?;

    let filepath = Path::new(&dirpath).join(filename);
    println!("File path {:?}", filepath);

    let status: Status;
    let mut content_type: Option<String> = None;

    match fs::write(&filepath, &req.body) {
        Ok(_) => {
            status = Status::Created;
            content_type = Some("application/octet-stream".to_string());
        }
        Err(e) => {
            println!(
                "Error: Unexpected error writing file: {:?}, err {}",
                filepath, e
            );
            status = Status::InternalServerError;
        }
    }

    Ok(Response {
        status,
        body: None,
        content_type,
    })
}
