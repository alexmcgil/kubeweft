use std::cmp::Reverse;

use kubeweft_model::DeviceId;

use crate::{Availability, PowerSource, Replica, ReplicaState, StorageNode};

pub struct PlacementRequest<'a> {
    pub size: u64,
    pub available_nodes: &'a [StorageNode],
    pub existing_replicas: &'a [Replica],
    pub desired_replica_count: usize,
}

pub trait PlacementPolicy: Send + Sync {
    /// Returns the desired nodes, preserving valid existing copies and adding
    /// the best currently-online candidates up to the requested durability.
    fn desired_nodes(&self, request: PlacementRequest<'_>) -> Vec<DeviceId>;
}

#[derive(Debug, Default)]
pub struct DefaultPlacementPolicy;

impl PlacementPolicy for DefaultPlacementPolicy {
    fn desired_nodes(&self, request: PlacementRequest<'_>) -> Vec<DeviceId> {
        let mut selected = request
            .existing_replicas
            .iter()
            .filter(|replica| {
                matches!(replica.state, ReplicaState::Healthy | ReplicaState::Offline)
            })
            .map(|replica| replica.node_id.clone())
            .collect::<Vec<_>>();
        selected.sort();
        selected.dedup();

        let mut candidates = request
            .available_nodes
            .iter()
            .filter(|node| {
                node.online
                    && node.free_space >= request.size
                    && !selected.contains(&node.device_id)
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|node| {
            (
                Reverse(matches!(node.availability, Availability::Stable)),
                Reverse(matches!(node.power, PowerSource::Mains)),
                Reverse(node.free_space),
                node.device_id.clone(),
            )
        });
        for node in candidates {
            if selected.len() >= request.desired_replica_count {
                break;
            }
            selected.push(node.device_id.clone());
        }
        selected
    }
}
