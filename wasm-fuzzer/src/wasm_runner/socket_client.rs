use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

static SOCKET_CLIENT: Lazy<SocketClient> = Lazy::new(|| {
    SocketClient::new("127.0.0.1:8080").expect("Failed to connect to server")
});

#[derive(Clone)]
pub struct SocketClient {
    stream: Arc<Mutex<TcpStream>>,
}

impl SocketClient {
    
    fn new(address: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
        })
    }

    pub fn instance() -> &'static SocketClient {
        &SOCKET_CLIENT
    }

    pub fn send(&self, data: &[u8]) -> std::io::Result<()> {
        let mut stream = self.stream.lock().unwrap();
        println!("Sending memory to validator");
        stream.write_all(&(data.len() as u64).to_be_bytes())?;
        stream.write_all(data)?;
        stream.flush()?;

        Ok(())
    }

    pub fn receive(&self) -> std::io::Result<String> {
        let mut stream = self.stream.lock().unwrap();
        let mut buffer = [0; 1024];
        let size = stream.read(&mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer[..size]).to_string())
    }
}
