mod partition;
mod partition_map;

use futures::Stream;
pub(crate) use partition::Partition;
pub use partition::{FalsePartition, TruePartition};
pub(crate) use partition_map::PartitionMap;
pub use partition_map::{FalsePartitionMap, PartitionBool, TruePartitionMap};

pub trait ParititonStream: Stream + Sized {
    fn partition<P>(
        self,
        predicate: P,
    ) -> (
        TruePartition<Self::Item, Self, P>,
        FalsePartition<Self::Item, Self, P>,
    )
    where
        P: Fn(&Self::Item) -> bool,
    {
        let stream = Partition::new(self, predicate);
        let true_stream = TruePartition::new(stream.clone());
        let false_stream = FalsePartition::new(stream);
        (true_stream, false_stream)
    }

    fn partition_map<T, F, P>(
        self,
        predicate: P,
    ) -> (
        TruePartitionMap<Self::Item, T, F, Self, P>,
        FalsePartitionMap<Self::Item, T, F, Self, P>,
    )
    where
        P: Fn(Self::Item) -> PartitionBool<T, F>,
    {
        let stream = PartitionMap::new(self, predicate);
        let true_stream = TruePartitionMap::new(stream.clone());
        let false_stream = FalsePartitionMap::new(stream);
        (true_stream, false_stream)
    }
}
