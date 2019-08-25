use std::net::{TcpListener, TcpStream};
use std::io::{self, Read, Write};
use std::{thread, time, str};

mod mongodb;

const BACKEND_ADDR: &str = "localhost:27017";

fn main() {
    let listen_addr = "127.0.0.1:27111";
    let listener = TcpListener::bind(listen_addr).unwrap();

    println!("Listening on {}", listen_addr);
    println!("^C to exit");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    match handle_connection(stream) {
                        Ok(_) => println!("closing connection."),
                        Err(e) => println!("connection error: {}", e),
                    };
                });
            },
            Err(e) => {
                println!("accept: {:?}", e)
            },
        }
    }
}

// Main proxy logic. Open a connection to the backend and start passing bytes
// between the client and the backend. Also split the traffic to MongoDb protocol
// parser, so that we can get some stats out of this.
//
// TODO: Convert this to async IO or some form of epoll (mio?)
fn handle_connection(mut client_stream: TcpStream) -> std::io::Result<()> {
    println!("new connection from {:?}", client_stream.peer_addr()?);
    println!("connecting to backend: {}", BACKEND_ADDR);
    let mut backend_stream = TcpStream::connect(BACKEND_ADDR)?;

    client_stream.set_nonblocking(true)?;
    backend_stream.set_nonblocking(true)?;

    let mut done = false;
    let mut client_parser = mongodb::MongoProtocolParser::new();
    let mut backend_parser = mongodb::MongoProtocolParser::new();

    while !done {
        let mut data_from_client = Vec::new();
        if !copy_stream(&mut client_stream, &mut backend_stream, &mut data_from_client)? {
            println!("{} client EOF", client_stream.peer_addr()?);
            done = true;
        }

        client_parser.parse_buffer(&data_from_client);

        let mut data_from_backend = Vec::new();
        if !copy_stream(&mut backend_stream, &mut client_stream, &mut data_from_backend)? {
            println!("{} backend EOF", backend_stream.peer_addr()?);
            done = true;
        }

        backend_parser.parse_buffer(&data_from_backend);

        thread::sleep(time::Duration::from_millis(1));
    }

    Ok(())
}

// Copy bytes from one stream to another. Return the processed bytes in buf.
//
// TODO: Use a user supplied buffer so that we're not unnecessarily 
// creating new vectors all the time.
//
// Return false on EOF
//
fn copy_stream(from_stream: &mut TcpStream, to_stream: &mut TcpStream,
               &mut buf: Vec<u8>) -> std::io::Result<bool> {

    buf.clear();
    match from_stream.read(&mut buf) {
        Ok(len) => {
            if len > 0 {
                to_stream.write_all(&buf)?;
            } else {
                return Ok(false);
            }
        },
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
        },
        Err(e) => {
            println!("error: {}", e);
        },
    }
    Ok(true)
}
