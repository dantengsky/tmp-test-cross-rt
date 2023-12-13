pub mod pb {
    tonic::include_proto!("grpc.examples.echo");
}

use std::time::Duration;
use tokio_stream::StreamExt;

use pb::{echo_client::EchoClient, EchoRequest};

use tokio::runtime::Runtime;
async fn test(simu_rpc_service_rt: &Runtime, simu_global_io_rt: &Runtime) {
    for i in 0..1000 {
        println!("iteration = {}", i);

        let mut stream = {
            // create stream from simu_rpc_service_rt
            let stream_fut = async {
                let mut client = EchoClient::connect("http://127.0.0.1:50051").await.unwrap();
                client
                    .server_streaming_echo(EchoRequest {
                        message: "foo".into(),
                    })
                    .await
                    .unwrap()
                    .into_inner()
            };

            simu_rpc_service_rt.spawn(stream_fut).await.unwrap()
        };

        // as  FlightClient::streaming_receiver does, use a bounded (cap = 1) async_channel to receive stream,
        // and forward it to the receiver
        let (tx, rx) = async_channel::bounded(1);

        let echo = async move {
            // let mut stream = stream.take(num);
            while let Some(item) = stream.next().await {
                if tx.send(item).await.is_err() {
                    break;
                }
                //println!("\treceived: {}", item.unwrap().message);
            }

            // as FlightClient::streaming_receiver does, drop the stream to close the connection explicitly
            drop(stream);
            tx.close();
        };

        // run it in the simu_global_io_rt
        simu_global_io_rt.spawn(echo);

        let receiver = async move {
            while let Ok(item) = rx.recv().await {
                // sleep 1s to avoid mess server println functions
                tokio::time::sleep(Duration::from_millis(15)).await;
                let _ = item;
                //println!("\treceived: {}", item.unwrap().message);
            }
        };

        // scenario 1: run receiver in the simu_rpc_service_rt
        // run reveiver in the simu_global_io_rt
        // simu_global_io_rt.spawn(receiver);

        // scenario 2: run receiver in an adhoc rt
        let adhoc_rt = Runtime::new().unwrap();
        adhoc_rt.spawn(receiver);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            drop(adhoc_rt);
        });
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let simulated_grpc_service_rt = Runtime::new().unwrap();
    let simulated_global_io_rt = Runtime::new().unwrap();
    let g = Runtime::new().unwrap();

    g.block_on(test(&simulated_grpc_service_rt, &simulated_global_io_rt));

    println!("main exiting...");

    {
        // thanks copilot...
        // read a key press and quit
        use std::io::{stdin, stdout, Read, Write};
        let mut stdout = stdout();
        stdout.write_all(b"Press any key to quit...").unwrap();
        stdout.flush().unwrap();
        let _ = stdin().read(&mut [0u8]).unwrap();
    }

    Ok(())
}
