use comms::event::UserMessageBroadcastEvent;
use comms::event::{self, Event};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

use super::{
    user_registry::UserRegistry, user_session_handle::UserSessionHandle, SessionAndUserId,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// [ChatRoomMetadata] holds the metadata that identifies a chat room
pub struct ChatRoomMetadata {
    pub name: String,
    pub description: String,
}

const BROADCAST_CHANNEL_CAPACITY: usize = 100;
const MAX_HISTORY: usize = 10;

#[derive(Debug)]
/// [ChatRoom] handles the participants of a chat room and the primary broadcast channel
/// A [UserSessionHandle] is handed out to a user when they join the room
pub struct ChatRoom {
    metadata: ChatRoomMetadata,
    broadcast_tx: broadcast::Sender<Event>,
    user_registry: UserRegistry,
    history: VecDeque<UserMessageBroadcastEvent>,
}

impl ChatRoom {
    pub fn new(metadata: ChatRoomMetadata) -> Self {
        let (broadcast_tx, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);

        ChatRoom {
            metadata,
            broadcast_tx,
            user_registry: UserRegistry::new(),
            history: VecDeque::with_capacity(MAX_HISTORY),
        }
    }

    pub fn get_unique_user_ids(&self) -> Vec<String> {
        self.user_registry.get_unique_user_ids()
    }

    /// Add a participant to the room and broadcast that they joined
    ///
    /// # Returns
    ///
    /// - A broadcast receiver for the user to receive messages from the room
    /// - A [UserSessionHandle] for the user to be able to interact with the room
    pub fn join(
        &mut self,
        session_and_user_id: &SessionAndUserId,
        chat_room_arc: Arc<Mutex<ChatRoom>>, // pass the chat room arc to the user session handle so it can record messages without broadcasting them
    ) -> (broadcast::Receiver<Event>, UserSessionHandle) {
        let broadcast_tx = self.broadcast_tx.clone();
        let broadcast_rx = broadcast_tx.subscribe();
        let user_session_handle = UserSessionHandle::new(
            self.metadata.name.clone(),
            broadcast_tx,
            session_and_user_id.clone(),
            chat_room_arc,
        );

        // If the user is new e.g. they do not have another session with same user id,
        // broadcast that they joined to all users
        if self.user_registry.insert(&user_session_handle) {
            let _ = self.broadcast_tx.send(Event::RoomParticipation(
                event::RoomParticipationBroadcastEvent {
                    user_id: session_and_user_id.user_id.clone(),
                    room: self.metadata.name.clone(),
                    status: event::RoomParticipationStatus::Joined,
                },
            ));
        }

        (broadcast_rx, user_session_handle)
    }

    /// Remove a participant from the room and broadcast that they left
    /// Consume the [UserSessionHandle] to drop it
    pub fn leave(&mut self, user_session_handle: UserSessionHandle) {
        if self.user_registry.remove(&user_session_handle) {
            let _ = self.broadcast_tx.send(Event::RoomParticipation(
                event::RoomParticipationBroadcastEvent {
                    user_id: String::from(user_session_handle.user_id()),
                    room: self.metadata.name.clone(),
                    status: event::RoomParticipationStatus::Left,
                },
            ));
        }
    }

    // Send a message to all users in the room and record it in the history
    pub fn send_message(&mut self, user_id: String, content: String) {
        let message_event = event::UserMessageBroadcastEvent {
            room: self.metadata.name.clone(),
            user_id,
            content,
        };

        self.history.push_back(message_event.clone());
        if self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }

        // broadcast
        let _ = self.broadcast_tx.send(Event::UserMessage(message_event));
    }

    // Record a message in the history without broadcasting it
    pub fn record_message(&mut self, message: &UserMessageBroadcastEvent) {
        self.history.push_back(message.clone());

        if self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
    }

    // Get the history of messages in the room
    pub fn get_history(&self) -> Vec<UserMessageBroadcastEvent> {
        self.history.iter().cloned().collect()
    }
}

#[test]
fn test_limit() {
    let metadata = ChatRoomMetadata {
        name: "test".into(),
        description: "desc".into(),
    };

    let mut room = ChatRoom::new(metadata);

    for i in 0..12 {
        let msg = event::UserMessageBroadcastEvent {
            room: "test".into(),
            user_id: "user".into(),
            content: format!("msg{}", i),
        };
        room.record_message(&msg);
    }

    assert_eq!(room.get_history().len(), 10);
}
