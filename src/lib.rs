mod split_by;
mod split_by_map;

pub(crate) use split_by::SplitBy;
pub use split_by::{FalseSplitBy, TrueSplitBy};
pub(crate) use split_by_map::SplitByMap;
pub use split_by_map::{LeftSplitByMap, RightSplitByMap};

pub use futures::future::Either;
use futures::Stream;

/// This is the extension crate would provides the functionality for splitting a stream
pub trait SplitByStreamExt: Stream {
    /// This takes ownership of a stream and returns two streams based on a predicate. When the predicate
    /// returns `true`, the item will appear in the first of the pair of streams returned.
    /// Items that return false will go into the second of the pair of streams
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

    /// This takes ownership of a stream and returns two streams based on a predicate. The predicate
    /// takes an item by value and returns `Either::Left(..)` or `Either::Right(..)` where the inner
    /// values of `Left` and `Right` become the items of the two respective streams
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

impl<T> SplitByStreamExt for T where T: Stream + ?Sized {}
