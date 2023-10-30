// Good references:
// - https://thepacketgeek.com/rust/tcpstream/reading-and-writing/

use std::io::BufReader;
use std::io::{BufRead, Write};
use std::net::{TcpListener, TcpStream};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_connection(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

// TODO: handle errors properly
fn handle_connection(mut stream: TcpStream) {
    // NOTE: We must read the data before writing any response,
    // otherwise the stream will automatically close the connection
    // and return "Recv failure: Connection reset by peer" to the client.

    // Wrap stream with Bufreader
    let mut reader = BufReader::new(&mut stream);

    // Read all data from this stream
    let received: Vec<u8> = reader.fill_buf().unwrap().to_vec();

    // Could run some validation based on the income data here...

    // Tells this buffer that amt bytes have been consumed from the buffer,
    // so they should no longer be returned in calls to read.
    reader.consume(received.len());

    // Write response:
    //
    // Respond with "HTTP/1.1 200 OK\r\n\r\n" (there are two \r\ns at the end)
    //
    // * "HTTP/1.1 200 OK" is the HTTP Status Line.
    // * "\r\n", also known as CRLF, is the end-of-line marker that HTTP uses.
    // * The first "\r\n" signifies the end of the status line.
    // * The second "\r\n" signifies the end of the response headers section (which is empty in this case).
    //
    let buf = "HTTP/1.1 200 OK\r\n\r\n".as_bytes();
    stream.write(buf).unwrap();
    stream.flush().unwrap();
}
