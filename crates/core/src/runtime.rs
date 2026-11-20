//! Application trait, side-effect commands and event subscriptions.

use crate::widget::Element;
use std::{future::Future, pin::Pin};

/// A boxed future that resolves to a message.
pub type BoxFuture<M> = Pin<Box<dyn Future<Output = M> + Send + 'static>>;

/// A side effect requested by `update`.
pub enum Command<M> {
    /// Do nothing.
    None,
    /// Run futures in the background; each result becomes a message.
    Perform(Vec<BoxFuture<M>>),
    /// Ask the runtime to close the window and exit.
    Exit,
}

impl<M> Command<M> {
    /// Run one future in the background.
    pub fn perform(fut: impl Future<Output = M> + Send + 'static) -> Self {
        Command::Perform(vec![Box::pin(fut)])
    }
}

/// A long-lived source of messages (timers, channels, OS events).
pub struct Subscription<M> {
    /// Stable identity so the runtime can diff subscriptions across updates.
    pub id: u64,
    /// Builds the message stream; real implementation lands in the runtime workstream.
    pub _marker: std::marker::PhantomData<M>,
}

/// The Elm-style application contract.
pub trait Application: Sized {
    /// Messages produced by widgets, commands and subscriptions.
    type Message: Send + 'static;

    /// Handle a message and return follow-up effects.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message>;
    /// Describe the UI for the current state.
    fn view(&self) -> Element<Self::Message>;
    /// Background event sources for the current state.
    fn subscription(&self) -> Vec<Subscription<Self::Message>> {
        Vec::new()
    }
}
