mod split_by;
mod split_by_map;

pub(crate) use split_by::SplitBy;
pub use split_by::{FalseSplitBy, TrueSplitBy};
pub(crate) use split_by_map::SplitByMap;
pub use split_by_map::{LeftSplitByMap, RightSplitByMap};

pub use either::Either;
use futures::Stream;

pub trait SplitByStreamExt: Stream {
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
