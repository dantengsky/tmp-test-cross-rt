pub mod pb {
    tonic::include_proto!("grpc.examples.echo");
}

use std::time::Duration;
use tokio_stream::{Stream, StreamExt};
use tonic::transport::Channel;

use pb::{echo_client::EchoClient, EchoRequest};

fn echo_requests_iter() -> impl Stream<Item = EchoRequest> {
    tokio_stream::iter(1..usize::MAX).map(|i| EchoRequest {
        message: format!("msg {:02}", i),
    })
}

async fn streaming_echo(client: &mut EchoClient<Channel>, num: usize) {
    let stream = client
        .server_streaming_echo(EchoRequest {
            message: "foo".into(),
        })
        .await
        .unwrap()
        .into_inner();

    // stream is infinite - take just 5 elements and then disconnect
    let mut stream = stream.take(num);
    while let Some(item) = stream.next().await {
        println!("\treceived: {}", item.unwrap().message);
    }
    // stream is droped here and the disconnect info is send to server
}

async fn bidirectional_streaming_echo(client: &mut EchoClient<Channel>, num: usize) {
    let in_stream = echo_requests_iter().take(num);

    let response = client
        .bidirectional_streaming_echo(in_stream)
        .await
        .unwrap();

    let mut resp_stream = response.into_inner();

    while let Some(received) = resp_stream.next().await {
        let received = received.unwrap();
        println!("\treceived message: `{}`", received.message);
    }
}

async fn bidirectional_streaming_echo_throttle(client: &mut EchoClient<Channel>, dur: Duration) {
    let in_stream = echo_requests_iter().throttle(dur);

    let response = client
        .bidirectional_streaming_echo(in_stream)
        .await
        .unwrap();

    let mut resp_stream = response.into_inner();

    while let Some(received) = resp_stream.next().await {
        let received = received.unwrap();
        println!("\treceived message: `{}`", received.message);
    }
}

use tokio::runtime::Runtime;
async fn test(rt1: &Runtime, rt2: &Runtime) {
    for i in 0..1000 {
        println!("i = {}", i);
        //        let client_ft = async { EchoClient::connect("http://[::1]:50051").await.unwrap() };

        // build client from global runtime rt1
        //        let mut client = rt1.block_on(client_ft);

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

        // build stream from global runtime rt2
        let mut stream = rt1.spawn(stream_fut).await.unwrap();

        //let num = 50;
        let (tx, rx) = async_channel::bounded(1);

        let echo = async move {
            // let mut stream = stream.take(num);
            while let Some(item) = stream.next().await {
                if tx.send(item).await.is_err() {
                    break;
                }
                //println!("\treceived: {}", item.unwrap().message);
            }
            drop(stream);
            tx.close();
        };

        rt2.spawn(echo);

        //let adhoc_rt = Runtime::new().unwrap();
        // sleep 1s to avoid mess server println functions

        let receiver = async move {
            while let Ok(item) = rx.recv().await {
                // sleep 1s to avoid mess server println functions
                tokio::time::sleep(Duration::from_millis(15)).await;
                let _ = item;
                //println!("\treceived: {}", item.unwrap().message);
            }
        };

        // this will leak
        //rt1.spawn(receiver);
        //
        rt2.spawn(receiver);

        //adhoc_rt.spawn(receiver);
        //std::thread::spawn(move || {
        //    std::thread::sleep(std::time::Duration::from_millis(30));
        //    drop(adhoc_rt);
        //});
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt1 = Runtime::new().unwrap();
    let rt2 = Runtime::new().unwrap();
    let g = Runtime::new().unwrap();

    g.block_on(test(&rt1, &rt2));

    eprintln!("main exiting...");

    // read a key press and quit
    use std::io::{stdin, stdout, Read, Write};
    let mut stdout = stdout();
    stdout.write_all(b"Press any key to quit...").unwrap();
    stdout.flush().unwrap();
    let _ = stdin().read(&mut [0u8]).unwrap();

    Ok(())
}
