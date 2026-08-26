use crate::error::*;
use iced::{
    Subscription,
    advanced::subscription::{self, EventStream, Hasher, Recipe},
    futures::{SinkExt, stream::BoxStream},
    stream,
};
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::watch;
use tracing::debug;

static NEXT_FIELD_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct Field<T> {
    id: u64,
    sender: watch::Sender<T>,
}

impl<T> Field<T> {
    pub fn new(value: T) -> Self {
        let (sender, _) = watch::channel(value);

        let id = NEXT_FIELD_ID.fetch_add(1, Ordering::Relaxed);
        debug!("Created new field with id {}", id);

        Self { id, sender }
    }

    pub fn get(&self) -> watch::Ref<'_, T> {
        self.sender.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<T> {
        self.sender.subscribe()
    }

    pub fn update(&self, value: T)
    where
        T: PartialEq,
    {
        debug!("Updating field with id {}", self.id);
        self.sender.send_if_modified(|current| {
            if *current == value {
                false
            } else {
                *current = value;
                true
            }
        });
    }

    pub fn subscription<M>(&self, map: fn(T) -> M) -> Subscription<M>
    where
        T: Clone + Send + Sync + 'static,
        M: Send + 'static,
    {
        let recipe = FieldRecipe {
            id: self.id,
            receiver: self.subscribe(),
            map,
        };

        subscription::from_recipe(recipe)
    }
}

struct FieldRecipe<T, M> {
    id: u64,
    map: fn(T) -> M,
    receiver: watch::Receiver<T>,
}

impl<T, M> Hash for FieldRecipe<T, M> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T, M> Recipe for FieldRecipe<T, M>
where
    T: Clone + Send + Sync + 'static,
    M: Send + 'static,
{
    type Output = M;

    fn hash(&self, state: &mut Hasher) {
        self.id.hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<'static, Self::Output> {
        let mut receiver = self.receiver;
        let map = self.map;
        debug!("Starting field subscription with id {}", self.id);
        let id = self.id;

        Box::pin(stream::channel(1, async move |mut output| {
            loop {
                if receiver.changed().await.is_err() {
                    let _ = Error::new("Field subscription closed unexpectedly");
                    break;
                }

                debug!("Field subscription with id {} received update", id);
                let value = receiver.borrow_and_update().clone();

                if output.send(map(value)).await.is_err() {
                    let _ = Error::new("Field subscription output closed unexpectedly");
                    break;
                }
            }
        }))
    }
}
