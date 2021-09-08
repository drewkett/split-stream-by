mod partition;
mod partition_map;

pub(crate) use partition::Partition;
pub use partition::{FalsePartition, TruePartition};
pub(crate) use partition_map::PartitionMap;
pub use partition_map::{LeftPartitionMap, RightPartitionMap};

pub use either::Either;
use futures::Stream;

pub trait PartitionStreamExt: Stream {
    fn split_by<P>(
        self,
        predicate: P,
    ) -> (
        TruePartition<Self::Item, Self, P>,
        FalsePartition<Self::Item, Self, P>,
    )
    where
        P: Fn(&Self::Item) -> bool,
        Self: Sized,
    {
        let stream = Partition::new(self, predicate);
        let true_stream = TruePartition::new(stream.clone());
        let false_stream = FalsePartition::new(stream);
        (true_stream, false_stream)
    }

    fn split_by_map<T, F, P>(
        self,
        predicate: P,
    ) -> (
        LeftPartitionMap<Self::Item, T, F, Self, P>,
        RightPartitionMap<Self::Item, T, F, Self, P>,
    )
    where
        P: Fn(Self::Item) -> Either<T, F>,
        Self: Sized,
    {
        let stream = PartitionMap::new(self, predicate);
        let true_stream = LeftPartitionMap::new(stream.clone());
        let false_stream = RightPartitionMap::new(stream);
        (true_stream, false_stream)
    }
}

impl<T> PartitionStreamExt for T where T: Stream + ?Sized {}
