use anyhow::Context;
use comms::event;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use super::ChatRoom;

#[derive(Debug, Clone)]

pub struct SessionAndUserId {
    pub session_id: String,
    pub user_id: String,
}

#[derive(Debug)]
/// [UserSessionHandle] is a handle that allows a specific user/session pair to
/// send messages to a specific room.
///
/// It is created when a user joins a room and is handed out to the user.
pub struct UserSessionHandle {
    /// The name of the room which is associated with this handle
    room: String,
    /// The channel to use for sending events to the all users of the room
    broadcast_tx: broadcast::Sender<event::Event>,
    /// The session and user id associated with this handle
    session_and_user_id: SessionAndUserId,

    pub(crate) chat_room: Arc<Mutex<ChatRoom>>,
}

impl UserSessionHandle {
    pub(super) fn new(
        room: String,
        broadcast_tx: broadcast::Sender<event::Event>,
        session_and_user_id: SessionAndUserId,
        chat_room: Arc<Mutex<ChatRoom>>,
    ) -> Self {
        UserSessionHandle {
            room,
            broadcast_tx,
            session_and_user_id,
            chat_room,
        }
    }

    pub fn room(&self) -> &str {
        &self.room
    }

    pub fn session_id(&self) -> &str {
        &self.session_and_user_id.session_id
    }

    pub fn user_id(&self) -> &str {
        &self.session_and_user_id.user_id
    }

    // Send a message to the room
    pub async fn send_message(&self, content: String) -> anyhow::Result<()> {
        let message_event = event::UserMessageBroadcastEvent {
            room: self.room.clone(),
            user_id: self.session_and_user_id.user_id.clone(),
            content,
        };

        {
            let mut room = self.chat_room.lock().await;
            room.record_message(&message_event);
        }

        self.broadcast_tx
            .send(event::Event::UserMessage(message_event))
            .context("could not write to the broadcast channel")?;

        Ok(())
    }
}
