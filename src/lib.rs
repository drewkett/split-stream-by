//!This crate offers a `futures::Stream` extension trait which allows for
//! splitting a `Stream` into two streams using a predicate function thats
//! checked on each `Stream::Item`.
//!
//!The current version of this crate buffers only one value and only in the
//! scenario where the item yielded from the parent stream is not what the child
//! stream requested per the predicate. In that scenario, the item is stored and
//! the other stream is awakened
//!
//!```rust
//! use split_stream_by::SplitStreamByExt;
//!
//! let incoming_stream = futures::stream::iter([0,1,2,3,4,5]);
//! let (mut even_stream, mut odd_stream) = incoming_stream.split_by(|&n| n % 2 == 0);
//!
//! tokio::spawn(async move {
//! 	while let Some(even_number) = even_stream.next().await {
//! 		println!("Even {}",even_number);
//! 	}
//! });
//!
//! while let Some(odd_number) = odd_stream.next().await {
//! 	println!("Odd {}",odd_number);
//! }
//! ```
//!
//!A more advanced usage uses `split_by_map` which allows for extracting values
//! while splitting
//!
//!```rust
//! use split_stream_by::{Either,SplitStreamByExt};
//!
//! struct Request {
//! 	//...
//! }
//!
//! struct Response {
//! 	//...
//! }
//!
//! enum Message {
//! 	Request(Request),
//! 	Response(Response)
//! }
//!
//! let incoming_stream = futures::stream::iter([
//! 	Message::Request(Request {}),
//! 	Message::Response(Response {}),
//! 	Message::Response(Response {}),
//! ]);
//! let (mut request_stream, mut response_stream) = incoming_stream.split_by_map(|item| match item {
//! 	Message::Request(req) => Either::Left(req),
//! 	Message::Response(res) => Either::Right(res),
//! });
//!
//! tokio::spawn(async move {
//! 	while let Some(request) = request_stream.next().await {
//! 		// ...
//! 	}
//! });
//!
//! while let Some(response) = response_stream.next().await {
//! 	// ...
//! }
//! ```
mod split_by;
mod split_by_map;

pub(crate) use split_by::SplitBy;
pub use split_by::{FalseSplitBy, TrueSplitBy};
pub(crate) use split_by_map::SplitByMap;
pub use split_by_map::{LeftSplitByMap, RightSplitByMap};

pub use futures::future::Either;
use futures::Stream;

/// This is the extension crate would provides the functionality for splitting a
/// stream
pub trait SplitStreamByExt: Stream {
    /// This takes ownership of a stream and returns two streams based on a
    /// predicate. When the predicate returns `true`, the item will appear in
    /// the first of the pair of streams returned. Items that return false will
    /// go into the second of the pair of streams
    ///
    ///```rust
    /// use split_stream_by::SplitStreamByExt;
    ///
    /// let incoming_stream = futures::stream::iter([0,1,2,3,4,5]);
    /// let (even_stream, odd_stream) = incoming_stream.split_by(|&n| n % 2 == 0);
    /// ```
    fn split_by<P>(
        self,
        predicate: P,
    ) -> (
        TrueSplitBy<Self::Item, Self, P>,
        FalseSplitBy<Self::Item, Self, P>,
    )
    where
        P: Fn(&Self::Item) -> bool,
        Self: Sized,
    {
        let stream = SplitBy::new(self, predicate);
        let true_stream = TrueSplitBy::new(stream.clone());
        let false_stream = FalseSplitBy::new(stream);
        (true_stream, false_stream)
    }

    /// This takes ownership of a stream and returns two streams based on a
    /// predicate. The predicate takes an item by value and returns
    /// `Either::Left(..)` or `Either::Right(..)` where the inner
    /// values of `Left` and `Right` become the items of the two respective
    /// streams
    ///
    /// ```
    /// use split_stream_by::{Either,SplitStreamByExt};
    /// struct Request {
    /// 	//...
    /// }
    /// struct Response {
    /// 	//...
    /// }
    /// enum Message {
    /// 	Request(Request),
    /// 	Response(Response)
    /// }
    /// let incoming_stream = futures::stream::iter([
    /// 	Message::Request(Request {}),
    /// 	Message::Response(Response {}),
    /// 	Message::Response(Response {}),
    /// ]);
    /// let (mut request_stream, mut response_stream) = incoming_stream.split_by_map(|item| match item {
    /// 	Message::Request(req) => Either::Left(req),
    /// 	Message::Response(res) => Either::Right(res),
    /// });
    /// ```

    fn split_by_map<L, R, P>(
        self,
        predicate: P,
    ) -> (
        LeftSplitByMap<Self::Item, L, R, Self, P>,
        RightSplitByMap<Self::Item, L, R, Self, P>,
    )
    where
        P: Fn(Self::Item) -> Either<L, R>,
        Self: Sized,
    {
        let stream = SplitByMap::new(self, predicate);
        let true_stream = LeftSplitByMap::new(stream.clone());
        let false_stream = RightSplitByMap::new(stream);
        (true_stream, false_stream)
    }
}

impl<T> SplitStreamByExt for T where T: Stream + ?Sized {}
