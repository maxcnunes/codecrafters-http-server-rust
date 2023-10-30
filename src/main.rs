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

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    http_info: String,
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
    };

    // Read all data from this stream
    for (i, l) in reader.lines().enumerate() {
        let line = l.unwrap();
        println!("line {:?}", line);
        if i == 0 {
            let parts: Vec<&str> = line.split(" ").collect();
            if parts.len() != 3 {
                panic!("Bad first line format {:?}", parts);
            }

            req.method = parts[0].to_string();
            req.path = parts[1].to_string();
            req.http_info = parts[2].to_string();
        }
        if line == "" {
            // End of the request data
            break;
        }
    }

    println!("Request {:?}", req);

    println!("Responding");

    // Write response:
    //
    // Respond with "HTTP/1.1 200 OK\r\n\r\n" (there are two \r\ns at the end)
    //
    // * "HTTP/1.1 200 OK" is the HTTP Status Line.
    // * "\r\n", also known as CRLF, is the end-of-line marker that HTTP uses.
    // * The first "\r\n" signifies the end of the status line.
    // * The second "\r\n" signifies the end of the response headers section (which is empty in this case).
    //
    let status = match req.path.as_str() {
        "/" => "200 OK",
        _ => "404 Not Found",
    };

    let res_content = format!("HTTP/1.1 {}\r\n\r\n", status);
    stream.write(&res_content.into_bytes()).unwrap();
    stream.flush().unwrap();
    println!("Done");
}
