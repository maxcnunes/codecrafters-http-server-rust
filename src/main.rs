// Good references:
// - https://thepacketgeek.com/rust/tcpstream/reading-and-writing/

use std::io::BufReader;
use std::io::{BufRead, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    println!("accepted new connection ({})", stream.peer_addr().unwrap());
                    handle_connection(stream);
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

enum Status {
    OK,
    NotFound,
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    http_info: String,
    headers: Vec<(String, String)>,
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
fn handle_connection(mut stream: TcpStream) {
    // NOTE: We must read the data before writing any response,
    // otherwise the stream will automatically close the connection
    // and return "Recv failure: Connection reset by peer" to the client.

    // Wrap stream with Bufreader
    let reader = BufReader::new(&mut stream);

    let mut req = Request {
        method: String::new(),
        path: String::new(),
        http_info: String::new(),
        headers: vec![],
    };

    // Read all data from this stream
    for (i, l) in reader.lines().enumerate() {
        let line = l.unwrap();
        println!("line {:?}", line);

        // parse request info
        if i == 0 {
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
            req.headers.push((parts.0.to_string(), parts.1.to_string()));
            continue;
        }

        if line == "" {
            // End of the request data
            break;
        }
    }

    println!("Request {:?}", req);

    println!("Responding");

    let mut res_body: Option<String> = None;

    let mut status: Status = Status::NotFound;

    // Handle routes
    if req.method == "GET" && req.path == "/" {
        status = Status::OK;
    } else if req.method == "GET" && req.path.starts_with("/echo/") {
        let parts: Vec<&str> = req.path.split("/").skip(2).collect();
        println!("Parts {:?}", parts);
        let param = parts.join("/");
        println!("Param {}", param);
        res_body = Some(param.to_string());
        status = Status::OK;
    } else if req.method == "GET" && req.path == "/user-agent" {
        res_body = req.get_header("User-Agent");
        status = Status::OK;
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
        Status::NotFound => "404 Not Found",
    };

    let mut res_content = format!("HTTP/1.1 {}\r\n", status_text);

    if let Some(body) = &res_body {
        res_content.push_str("Content-Type: text/plain\n");

        let cont_len = format!("Content-Length: {}\n", body.len());
        res_content.push_str(&cont_len.to_string());
    } else {
        res_content.push_str("\r\n");
    }

    res_content.push_str("\r\n");

    if let Some(body) = &res_body {
        res_content.push_str(&body);
    }
    println!("{}", res_content);

    stream.write(&res_content.into_bytes()).unwrap();
    stream.flush().unwrap();
    println!("Done");
}
