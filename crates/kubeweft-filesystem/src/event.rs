use std::sync::Mutex;

use kubeweft_model::DeviceId;

use crate::{ContentId, FileId, FileVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemEvent {
    FileCreated {
        path: String,
        file_id: FileId,
    },
    FileVersionCommitted {
        file_id: FileId,
        generation: FileVersion,
        content_id: ContentId,
    },
    ReplicaCreated {
        content_id: ContentId,
        node_id: DeviceId,
    },
    ReplicaUnavailable {
        content_id: ContentId,
        node_id: DeviceId,
    },
    ReplicaCorrupted {
        content_id: ContentId,
        node_id: DeviceId,
    },
    ReplicationStarted {
        content_id: ContentId,
        source: DeviceId,
        destination: DeviceId,
    },
    ReplicationCompleted {
        content_id: ContentId,
        source: DeviceId,
        destination: DeviceId,
    },
    DurabilityDegraded {
        content_id: ContentId,
    },
    DurabilityRestored {
        content_id: ContentId,
    },
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: FilesystemEvent);
}

#[derive(Debug, Default)]
pub struct RecordingEventSink {
    events: Mutex<Vec<FilesystemEvent>>,
}

impl RecordingEventSink {
    pub fn events(&self) -> Vec<FilesystemEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl EventSink for RecordingEventSink {
    fn emit(&self, event: FilesystemEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

pub(crate) struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: FilesystemEvent) {}
}
