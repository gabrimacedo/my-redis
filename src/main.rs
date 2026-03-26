use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("localhost:6379").await.unwrap();
}
