use futures::StreamExt;
use split_stream_by::{Either, SplitStreamByMapExt};
use tokio::join;

#[derive(Debug)]
struct Request {
    //...
}
#[derive(Debug)]
struct Response {
    //...
}
#[derive(Debug)]
enum Message {
    Request(Request),
    Response(Response),
}

#[tokio::main]
async fn main() {
    let incoming_stream = futures::stream::iter([
        Message::Request(Request {}),
        Message::Response(Response {}),
        Message::Response(Response {}),
    ]);

    let (request_stream, response_stream) = incoming_stream.split_by_map(|item| match item {
        Message::Request(req) => Either::Left(req),
        Message::Response(res) => Either::Right(res),
    });

    let a = tokio::spawn(async {
        println!("Requests: {:?}", request_stream.collect::<Vec<_>>().await);
    });
    let b = tokio::spawn(async {
        println!("Responses: {:?}", response_stream.collect::<Vec<_>>().await);
    });
    let _ = join!(a, b);
}
