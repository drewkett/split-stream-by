This repo is for a rust crate that offers a `futures::Stream` extension
trait which allows for splitting a `Stream` into two streams using a
predicate function that is checked on each `Stream::Item`.

```rust
use split_stream_by::SplitStreamByExt;

let incoming_stream = futures::stream::iter([0,1,2,3,4,5]);
let (mut even_stream, mut odd_stream) = incoming_stream.split_by(|&n| n % 2 == 0);

tokio::spawn(async move {
	while let Some(even_number) = even_stream.next().await {
		println!("Even {}",even_number);
	}
});

while let Some(odd_number) = odd_stream.next().await {
	println!("Odd {}",odd_number);
}
```

A more advanced usage uses `split_by_map` which allows for extracting
values while splitting

```rust
use split_stream_by::{Either,SplitStreamByExt};

struct Request {
	//...
}

struct Response {
	//...
}

enum Message {
	Request(Request),
	Response(Response)
}

let incoming_stream = futures::stream::iter([
	Message::Request(Request {}),
	Message::Response(Response {}),
	Message::Response(Response {}),
]);
let (mut request_stream, mut response_stream) = incoming_stream.split_by_map(|item| match item {
	Message::Request(req) => Either::Left(req),
	Message::Response(res) => Either::Right(res),
});

tokio::spawn(async move {
	while let Some(request) = request_stream.next().await {
		// ...
	}
});

while let Some(response) = response_stream.next().await {
	// ...
}
```
`split_by` and `split_by_map` only buffer up to 1 element. Use `split_by_buffered` or `split_by_map_buffered` to customize the number of buffered elements.
