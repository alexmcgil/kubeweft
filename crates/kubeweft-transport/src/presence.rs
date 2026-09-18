use std::{collections::BTreeMap, sync::Mutex};

use kubeweft_model::DeviceId;

use crate::{ClusterMember, PresenceRecord, PresenceState, TransportError, domain::now_millis};

#[derive(Default)]
pub(crate) struct PresenceTable {
    records: Mutex<BTreeMap<DeviceId, PresenceRecord>>,
}

impl PresenceTable {
    pub(crate) fn sync_members(
        &self,
        members: &[ClusterMember],
        local_device_id: &DeviceId,
    ) -> Result<(), TransportError> {
        let mut records = self.records.lock().map_err(|_| TransportError::Conflict)?;
        records.retain(|device_id, _| members.iter().any(|member| member.device_id == *device_id));
        for member in members {
            let default_state = if member.device_id == *local_device_id {
                PresenceState::Online
            } else {
                PresenceState::Unknown
            };
            let record =
                records
                    .entry(member.device_id.clone())
                    .or_insert_with(|| PresenceRecord {
                        member: member.clone(),
                        state: default_state,
                        observed_at_millis: (default_state == PresenceState::Online)
                            .then(now_millis),
                    });
            record.member = member.clone();
            if member.device_id == *local_device_id {
                record.state = PresenceState::Online;
                record.observed_at_millis = Some(now_millis());
            }
        }
        Ok(())
    }

    pub(crate) fn observe(
        &self,
        member: &ClusterMember,
        state: PresenceState,
    ) -> Result<(), TransportError> {
        let mut records = self.records.lock().map_err(|_| TransportError::Conflict)?;
        records.insert(
            member.device_id.clone(),
            PresenceRecord {
                member: member.clone(),
                state,
                observed_at_millis: Some(now_millis()),
            },
        );
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Result<Vec<PresenceRecord>, TransportError> {
        Ok(self
            .records
            .lock()
            .map_err(|_| TransportError::Conflict)?
            .values()
            .cloned()
            .collect())
    }
}
