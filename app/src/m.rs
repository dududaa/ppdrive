// process_b.rs
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // channel to signal cancellation
    let (tx, mut rx) = watch::channel(false);

    // Start the worker task
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = rx.changed() => {
                    if *rx.borrow() {
                        println!("Task received stop signal, exiting...");
                        break;
                    }
                }
                _ = sleep(Duration::from_secs(1)) => {
                    println!("Working...");
                }
            }
        }
    });

    // Listen for control messages on TCP (localhost:5000)
    let listener = TcpListener::bind("127.0.0.1:5000").await?;
    println!("Process B listening on 127.0.0.1:5000");

    loop {
        let (mut socket, _) = listener.accept().await?;
        let tx = tx.clone();

        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;

            let mut buf = [0u8; 1024];
            if let Ok(n) = socket.read(&mut buf).await {
                if n > 0 {
                    let msg = String::from_utf8_lossy(&buf[..n]);
                    println!("Received control message: {}", msg);

                    if msg.trim() == "STOP" {
                        let _ = tx.send(true);
                    }
                }
            }
        });
    }
}


// process_a.rs
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:5000").await?;
    stream.write_all(b"STOP").await?;
    println!("Sent STOP signal to Process B");
    Ok(())
}
